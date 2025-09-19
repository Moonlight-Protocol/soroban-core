use crate::core::{calculate_auth_requirements, InternalBundle, UTXOOperation, UtxoHandlerTrait};

use soroban_sdk::{
    auth::{Context, CustomAccountInterface},
    contract, contracterror, contractimpl,
    crypto::Hash,
    panic_with_error, vec, Address, BytesN, Env, Vec,
};

#[contract]
pub struct UTXOModuleTestContract;

#[contractimpl]
impl UtxoHandlerTrait for UTXOModuleTestContract {}

#[contractimpl]
impl UTXOModuleTestContract {
    pub fn __constructor(e: Env, utxo_auth: Address) {
        Self::set_auth(&e, &utxo_auth);
    }

    pub fn transact(e: Env, op: UTXOOperation) {
        Self::transact_with_external(e, op, 0, 0);
    }

    pub fn transact_with_external(
        e: Env,
        op: UTXOOperation,
        incoming_amount: i128,
        outgoing_amount: i128,
    ) -> i128 {
        let mut spend: Vec<BytesN<65>> = vec![&e];
        let mut create: Vec<(BytesN<65>, i128)> = vec![&e];

        for (spend_utxo, _conditions) in op.spend.iter() {
            spend.push_back(spend_utxo.clone());
        }
        for create_utxo in op.create.iter() {
            create.push_back(create_utxo.clone());
        }

        let req = calculate_auth_requirements(&e, &op.spend); // &vec![&e]);

        let bundle: InternalBundle = InternalBundle {
            spend: spend,
            create: create,
            req,
        };

        Self::process_bundle(&e, bundle, incoming_amount, outgoing_amount)
    }

    pub fn mint(e: Env, utxos: Vec<(BytesN<65>, i128)>) {
        for (utxo, amount) in utxos {
            Self::create(&e, amount, utxo);
        }
    }

    pub fn burn(e: Env, utxos: Vec<BytesN<65>>) {
        for utxo in utxos {
            Self::spend(&e, utxo);
        }
    }
}

pub fn create_contract(e: &Env, auth: Address) -> (UTXOModuleTestContractClient, Address) {
    let contract_id = e.register(
        UTXOModuleTestContract,
        UTXOModuleTestContractArgs::__constructor(&auth),
    );
    let contract = UTXOModuleTestContractClient::new(e, &contract_id);
    // Initialize contract if needed
    (contract, contract_id)
}

#[contract]
pub struct MockedAuthContract;

#[contractimpl]
impl CustomAccountInterface for MockedAuthContract {
    type Error = AuthError;
    type Signature = bool;

    fn __check_auth(
        e: Env,
        _payload: Hash<32>,      // used for provider auth
        signature: bool,         // provided by tx submitter in Authorization entry
        _contexts: Vec<Context>, // require_auth_for_args
    ) -> Result<(), AuthError> {
        if !signature {
            panic_with_error!(&e, AuthError::AuthorizationFailed);
        }

        Ok(())
    }
}

#[contracterror]
pub enum AuthError {
    AuthorizationFailed = 1,
}

pub fn create_contract_with_mocked_auth(
    e: &Env,
) -> (UTXOModuleTestContractClient, MockedAuthContractClient) {
    let auth_contract_id = e.register(MockedAuthContract, ());
    let auth = MockedAuthContractClient::new(e, &auth_contract_id);

    let contract_id = e.register(
        UTXOModuleTestContract,
        UTXOModuleTestContractArgs::__constructor(&auth_contract_id),
    );
    let contract = UTXOModuleTestContractClient::new(e, &contract_id);
    // Initialize contract if needed
    (contract, auth)
}
