use core::ops::{Deref, DerefMut};

use moonlight_helpers::testutils::keys::AccountEd25519Signature;
use moonlight_primitives::{
    equal_condition_sequence, has_no_conflicting_conditions_in_sets, hash_payload, AuthPayload,
    Condition,
};
use moonlight_utxo_core::testutils::operation_bundle::UTXOOperationBuilder;
use soroban_sdk::{
    contracttype,
    crypto::Hash,
    vec,
    xdr::{self, HashIdPreimage, HashIdPreimageSorobanAuthorization, Limits, VecM, WriteXdr},
    Address, Bytes, Env, IntoVal, Map, TryIntoVal, Val, Vec,
};

use crate::transact::ChannelOperation;

#[derive(Clone)]
#[contracttype]
pub struct ChannelOperationBuilder {
    utxo_builder: UTXOOperationBuilder,
    deposit: Vec<(Address, i128, Vec<Condition>)>,
    withdraw: Vec<(Address, i128, Vec<Condition>)>,
    asset: Address,
    deposit_sign_map: Map<Address, AccountEd25519Signature>,
}

impl ChannelOperationBuilder {
    pub fn generate(
        e: &Env,
        channel_contract: Address,
        auth_contract: Address,
        asset: Address,
    ) -> Self {
        let deposit = Vec::new(e);
        let withdraw = Vec::new(e);
        let deposit_sign_map = Map::new(&e);
        Self {
            utxo_builder: UTXOOperationBuilder::generate(e, channel_contract, auth_contract),
            deposit,
            withdraw,
            asset,
            deposit_sign_map,
        }
    }

    pub fn get_deposit(&self) -> Vec<(Address, i128, Vec<Condition>)> {
        self.deposit.clone()
    }

    pub fn get_withdraw(&self) -> Vec<(Address, i128, Vec<Condition>)> {
        self.withdraw.clone()
    }

    pub fn add_deposit(
        &mut self,
        e: &Env,
        address: Address,
        amount: i128,
        conditions: Vec<Condition>,
    ) {
        for (existing_address, _, existing_conditions) in self.deposit.iter() {
            if existing_address == address {
                panic!("Deposit already included for address");
            }

            assert!(
                has_no_conflicting_conditions_in_sets(&conditions, &existing_conditions),
                "New deposit conditions conflict with existing deposit conditions"
            );
        }

        for (existing_address, _, existing_conditions) in self.withdraw.iter() {
            if existing_address == address {
                // conditions must be exact equal
                if !equal_condition_sequence(&e, &existing_conditions, &conditions) {
                    panic!(
                        "Conditions don't match conditions for this address in the withdraw list"
                    );
                }
            }

            assert!(
                has_no_conflicting_conditions_in_sets(&conditions, &existing_conditions),
                "New deposit conditions conflict with existing withdraw conditions"
            );
        }

        self.deposit.push_back((address, amount, conditions));
    }
    pub fn add_withdraw(
        &mut self,
        e: &Env,
        address: Address,
        amount: i128,
        conditions: Vec<Condition>,
    ) {
        for (existing_address, _, existing_conditions) in self.withdraw.iter() {
            if existing_address == address {
                panic!("Withdraw already included for address");
            }

            assert!(
                has_no_conflicting_conditions_in_sets(&conditions, &existing_conditions),
                "New withdraw conditions conflict with existing withdraw conditions"
            );
        }

        for (existing_address, _, existing_conditions) in self.deposit.iter() {
            if existing_address == address {
                // conditions must be exact equal
                if !equal_condition_sequence(&e, &existing_conditions, &conditions) {
                    panic!(
                        "Conditions don't match conditions for this address in the deposit list"
                    );
                }
            }
            assert!(
                has_no_conflicting_conditions_in_sets(&conditions, &existing_conditions),
                "New withdraw conditions conflict with existing deposit conditions"
            );
        }

        self.withdraw.push_back((address, amount, conditions));
    }

    pub fn get_operation_bundle(&self) -> ChannelOperation {
        ChannelOperation {
            spend: self.get_spend(),
            create: self.get_create(),
            deposit: self.deposit.clone(),
            withdraw: self.withdraw.clone(),
        }
    }

    pub fn get_auth_payload_for_address(
        &self,
        address: Address,
        live_until_ledger: u32,
    ) -> AuthPayload {
        for (existing_address, _, conditions) in self.deposit.iter() {
            if existing_address == address {
                return AuthPayload {
                    contract: self.utxo_builder.channel_contract.clone(),
                    conditions: conditions.clone(),
                    live_until_ledger,
                };
            }
        }

        for (existing_address, _, conditions) in self.withdraw.iter() {
            if existing_address == address {
                return AuthPayload {
                    contract: self.utxo_builder.channel_contract.clone(),
                    conditions: conditions.clone(),
                    live_until_ledger,
                };
            }
        }

        panic!("Address not found in deposit or withdraw list");
    }

    pub fn get_auth_hash_for_address(
        &self,
        e: &Env,
        address: Address,
        live_until_ledger: u32,
    ) -> Hash<32> {
        let payload = self.get_auth_payload_for_address(address, live_until_ledger);
        hash_payload(&e, &payload)
    }

    pub fn add_deposit_signature(
        &mut self,
        public_key: Address,
        signature: AccountEd25519Signature,
    ) {
        assert!(
            !self.has_deposit_signature_for_address(&public_key),
            "Signature for this depositor address already added"
        );

        self.deposit_sign_map.set(public_key, signature);
    }

    pub fn get_contract_deposit_auth_args(
        &self,
        e: &Env,
        depositor: Address,
    ) -> (Vec<Val>, Vec<Val>) {
        let (deposit_conditions, amount) = self
            .deposit
            .iter()
            .find_map(|(addr, amt, conds)| {
                if addr == depositor {
                    Some((conds, amt))
                } else {
                    None
                }
            })
            .expect("Depositor address not found in deposit list");

        // CHANNEL TRANSACT with ARGS: [DEPOSIT_CONDITIONS]
        let root_args: Vec<Val> = vec![
            &e,
            deposit_conditions
                .try_into_val(e)
                .unwrap_or_else(|_| panic!("intoval")),
        ];

        // TRANSFER: From, To, Amount
        let sub_args: Vec<Val> = vec![
            &e,
            depositor.clone().into_val(e),
            self.channel_contract.clone().into_val(e),
            amount.into_val(e),
        ];

        (root_args, sub_args)
    }

    pub fn get_deposit_root_invocation(
        &self,
        e: &Env,
        depositor: Address,
    ) -> xdr::SorobanAuthorizedInvocation {
        let (root_args, sub_args) = self.get_contract_deposit_auth_args(&e, depositor);

        let root_invocation = xdr::SorobanAuthorizedInvocation {
            function: xdr::SorobanAuthorizedFunction::ContractFn(xdr::InvokeContractArgs {
                contract_address: self.channel_contract.clone().try_into().unwrap(),
                function_name: "transact".try_into().unwrap(),
                args: root_args.try_into().unwrap(),
            }),
            sub_invocations: VecM::try_from([xdr::SorobanAuthorizedInvocation {
                function: xdr::SorobanAuthorizedFunction::ContractFn(xdr::InvokeContractArgs {
                    contract_address: self.asset.clone().try_into().unwrap(),
                    function_name: "transfer".try_into().unwrap(),
                    args: sub_args.try_into().unwrap(),
                }),
                sub_invocations: VecM::default(),
            }])
            .unwrap(),
        };

        root_invocation
    }

    pub fn get_auth_entry_payload_hash_for_deposit(
        &self,
        e: &Env,
        depositor: Address,
        nonce: i64,
        signature_expiration_ledger: u32,
    ) -> Hash<32> {
        let payload = HashIdPreimage::SorobanAuthorization(HashIdPreimageSorobanAuthorization {
            network_id: e.ledger().network_id().to_array().into(),
            nonce,
            signature_expiration_ledger,
            invocation: self.get_deposit_root_invocation(&e, depositor.clone()),
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

    pub fn get_auth_entry_for_deposit(
        &self,
        e: &Env,
        depositor: Address,
        nonce: i64,
        signature_expiration_ledger: u32,
    ) -> xdr::SorobanAuthorizationEntry {
        let depositor_address_val = depositor
            .clone()
            .try_into()
            .unwrap_or_else(|_| panic!("intoval"));

        let signature_val = vec![&e, self.get_deposit_signature(&depositor)];

        let root_invocation = self
            .get_deposit_root_invocation(&e, depositor)
            .try_into()
            .unwrap_or_else(|_| panic!("intoval"));

        xdr::SorobanAuthorizationEntry {
            credentials: xdr::SorobanCredentials::Address(xdr::SorobanAddressCredentials {
                address: depositor_address_val,
                nonce,
                signature_expiration_ledger,
                signature: signature_val
                    .try_into()
                    .unwrap_or_else(|_| panic!("intoval")), // vec of Ed25519 signatures
            }),
            root_invocation,
        }
    }
    fn has_deposit_signature_for_address(&self, public_key: &Address) -> bool {
        self.deposit_sign_map.contains_key(public_key.clone())
    }

    fn get_deposit_signature(&self, public_key: &Address) -> AccountEd25519Signature {
        self.deposit_sign_map
            .get(public_key.clone())
            .unwrap_or_else(|| panic!("No signature found for this depositor address"))
    }
}

impl Deref for ChannelOperationBuilder {
    type Target = UTXOOperationBuilder;
    fn deref(&self) -> &Self::Target {
        &self.utxo_builder
    }
}

impl DerefMut for ChannelOperationBuilder {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.utxo_builder
    }
}
