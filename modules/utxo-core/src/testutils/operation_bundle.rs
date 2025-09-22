use moonlight_helpers::parser::address_to_ed25519_pk_bytes;
use moonlight_primitives::{
    condition_does_not_conflict_with_set, has_no_conflicting_conditions_in_sets, hash_payload,
    AuthPayload, AuthRequirements, Condition, Signature, Signatures, SignerKey,
};
use soroban_sdk::{
    contracttype,
    crypto::Hash,
    vec,
    xdr::{
        self, HashIdPreimage, HashIdPreimageSorobanAuthorization, Limits,
        SorobanAddressCredentials, VecM, WriteXdr,
    },
    Address, Bytes, BytesN, Env, Map, TryIntoVal, Val, Vec,
};

use crate::core::{calculate_auth_requirements, UTXOOperation};

#[derive(Clone)]
#[contracttype]
pub struct UTXOOperationBuilder {
    pub channel_contract: Address,
    pub auth_contract: Address,
    spend: Vec<(BytesN<65>, Vec<Condition>)>,
    create: Vec<(BytesN<65>, i128)>,
    sign_map: Map<SignerKey, (Signature, u32)>,
}

impl UTXOOperationBuilder {
    pub fn generate(
        e: &Env,
        channel_contract: Address,
        auth_contract: Address,
    ) -> UTXOOperationBuilder {
        let spend = Vec::new(e);
        let create = Vec::new(e);
        let sign_map = Map::new(&e);
        UTXOOperationBuilder {
            channel_contract,
            auth_contract,
            spend,
            create,
            sign_map,
        }
    }

    pub fn get_operation_bundle(&self) -> UTXOOperation {
        UTXOOperation {
            spend: self.spend.clone(),
            create: self.create.clone(),
        }
    }

    pub fn get_spend(&self) -> Vec<(BytesN<65>, Vec<Condition>)> {
        self.spend.clone()
    }

    pub fn get_create(&self) -> Vec<(BytesN<65>, i128)> {
        self.create.clone()
    }

    pub fn add_spend(&mut self, utxo: BytesN<65>, conditions: Vec<Condition>) {
        for (existing_utxo, existing_conditions) in self.spend.iter() {
            if existing_utxo == utxo {
                panic!("UTXO already included in spend list");
            }

            assert!(
                has_no_conflicting_conditions_in_sets(&conditions, &existing_conditions,),
                "Conflicting conditions with existing spend UTXO",
            );
        }

        self.spend.push_back((utxo, conditions));
    }

    pub fn add_create(&mut self, utxo: BytesN<65>, amount: i128) {
        for (existing_utxo, _amount) in self.create.iter() {
            if existing_utxo == utxo {
                panic!("UTXO already included in create list");
            }
        }

        let condition = Condition::Create(utxo.clone(), amount);

        for (_, existing_conditions) in self.spend.iter() {
            assert!(
                condition_does_not_conflict_with_set(&condition, &existing_conditions),
                "Create condition conflicts with existing spend UTXO",
            );
        }

        self.create.push_back((utxo, amount));
    }

    pub fn get_auth_payload_for_spend(
        &self,
        utxo: BytesN<65>,
        live_until_ledger: u32,
    ) -> AuthPayload {
        for (existing_utxo, conditions) in self.spend.iter() {
            if existing_utxo == utxo {
                return AuthPayload {
                    contract: self.channel_contract.clone(),
                    conditions: conditions.clone(),
                    live_until_ledger,
                };
            }
        }
        panic!("UTXO not found in spend list");
    }

    pub fn get_auth_hash_for_spend(
        &self,
        e: &Env,
        utxo: BytesN<65>,
        live_until_ledger: u32,
    ) -> Hash<32> {
        let payload = self.get_auth_payload_for_spend(utxo, live_until_ledger);
        hash_payload(&e, &payload)
    }

    pub fn add_spend_signature(
        &mut self,
        e: &Env,
        spend_utxo: BytesN<65>,
        signature: [u8; 64],
        live_until_ledger: u32,
    ) {
        assert!(
            !self.has_signature_for_spend_utxo(&spend_utxo),
            "Signature for this UTXO already added"
        );

        let signature_bytes = BytesN::<64>::from_array(&e, &signature);

        self.sign_map.set(
            SignerKey::P256(spend_utxo),
            (Signature::P256(signature_bytes), live_until_ledger),
        );
    }

    pub fn add_ed25519_signature(
        &mut self,
        public_key: BytesN<32>,
        signature: BytesN<64>,
        live_until_ledger: u32,
    ) {
        assert!(
            !self.has_signature_for_ed25519(&public_key),
            "Signature for this Ed25519 key already added"
        );

        self.sign_map.set(
            SignerKey::Ed25519(public_key),
            (Signature::Ed25519(signature), live_until_ledger),
        );
    }

    pub fn add_provider_signature(
        &mut self,
        e: &Env,
        provider_address: Address,
        signature: BytesN<64>,
        live_until_ledger: u32,
    ) {
        // revisit if we add multi-sig provider support
        assert!(
            !self.has_signature_for_provider(),
            "Provider signature already included"
        );

        let provider_bytes = address_to_ed25519_pk_bytes(&e, &provider_address).into();

        self.sign_map.set(
            SignerKey::Provider(provider_bytes),
            (Signature::Ed25519(signature), live_until_ledger),
        );
    }

    pub fn build_signatures(&self) -> Signatures {
        assert!(
            self.has_all_required_spend_signatures(),
            "Not all required spend signatures have been added"
        );
        assert!(
            self.has_signature_for_provider(),
            "Provider signature is required but not included"
        );

        Signatures(self.sign_map.clone())
    }

    pub fn calculate_auth_requirements(&self, e: &Env) -> AuthRequirements {
        calculate_auth_requirements(&e, &self.spend.clone()) // &vec![&e])
    }

    pub fn get_contract_auth_args(&self, e: &Env) -> Vec<Val> {
        let auth_req = self.calculate_auth_requirements(&e);
        let args: Vec<Val> = vec![
            &e,
            auth_req
                .try_into_val(e)
                .unwrap_or_else(|_| panic!("intoval")),
        ];
        args
    }

    pub fn get_root_invocation(&self, e: &Env) -> xdr::SorobanAuthorizedInvocation {
        let root_invocation = xdr::SorobanAuthorizedInvocation {
            function: xdr::SorobanAuthorizedFunction::ContractFn(xdr::InvokeContractArgs {
                contract_address: self.channel_contract.clone().try_into().unwrap(),
                function_name: "transact".try_into().unwrap(),
                args: self.get_contract_auth_args(&e).try_into().unwrap(), //VecM::try_from(vec![&e, invocation_args.clone()]).unwrap(),
            }),
            sub_invocations: VecM::default(),
        };
        root_invocation
    }

    pub fn get_auth_entry_payload_hash_for_bundle(
        &self,
        e: &Env,
        nonce: i64,
        signature_expiration_ledger: u32,
    ) -> Hash<32> {
        let payload = HashIdPreimage::SorobanAuthorization(HashIdPreimageSorobanAuthorization {
            network_id: e.ledger().network_id().to_array().into(),
            nonce,
            signature_expiration_ledger,
            invocation: self.get_root_invocation(&e),
        });

        let payload_xdr = payload
            .to_xdr(Limits {
                depth: u32::MAX,
                len: usize::MAX,
            })
            .unwrap();

        let mut payload_bytes = Bytes::new(&e);

        for &byte in payload_xdr.iter() {
            payload_bytes.push_back(byte);
        }

        let hash = e.crypto().sha256(&payload_bytes);

        hash
    }

    pub fn get_auth_entry(
        &self,
        e: &Env,
        nonce: i64,
        signature_expiration_ledger: u32,
    ) -> xdr::SorobanAuthorizationEntry {
        let contract_address_val = self
            .auth_contract
            .clone()
            .try_into()
            .unwrap_or_else(|_| panic!("intoval"));

        let signature_val = self
            .build_signatures()
            .try_into()
            .unwrap_or_else(|_| panic!("intoval"));

        let root_invocation = self.get_root_invocation(&e);

        xdr::SorobanAuthorizationEntry {
            credentials: xdr::SorobanCredentials::Address(SorobanAddressCredentials {
                address: contract_address_val,
                nonce,
                signature_expiration_ledger,
                signature: signature_val,
            }),
            root_invocation,
        }
    }

    fn has_signature_for_ed25519(&self, public_key: &BytesN<32>) -> bool {
        self.sign_map
            .contains_key(SignerKey::Ed25519(public_key.clone()))
    }

    fn has_signature_for_spend_utxo(&self, spend_utxo: &BytesN<65>) -> bool {
        self.sign_map
            .contains_key(SignerKey::P256(spend_utxo.clone()))
    }

    fn has_signature_for_provider(&self) -> bool {
        for key in self.sign_map.keys() {
            if let SignerKey::Provider(_) = key {
                return true;
            }
        }
        false
    }

    fn has_all_required_spend_signatures(&self) -> bool {
        for (spend_utxo, _conditions) in self.spend.iter() {
            if !self.has_signature_for_spend_utxo(&spend_utxo) {
                return false;
            }
        }
        true
    }
}
