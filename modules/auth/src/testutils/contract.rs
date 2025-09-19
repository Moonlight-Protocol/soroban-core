use moonlight_primitives::Signatures;
use soroban_sdk::{
    auth::{Context, CustomAccountInterface},
    contract, contractimpl,
    crypto::Hash,
    Address, Env, Vec,
};

use crate::core::{Error as AuthError, ProviderAuthorizable, UtxoAuthorizable};

#[contract]
pub struct AuthModuleTestContract;

#[contractimpl]
impl UtxoAuthorizable for AuthModuleTestContract {}

#[contractimpl]
impl ProviderAuthorizable for AuthModuleTestContract {}

#[contractimpl]
impl AuthModuleTestContract {
    pub fn add_provider(e: &Env, provider: Address) {
        Self::register_provider(e, provider);
    }

    pub fn remove_provider(e: &Env, provider: Address) {
        Self::deregister_provider(e, provider);
    }
}

#[contractimpl]
impl CustomAccountInterface for AuthModuleTestContract {
    type Error = AuthError;
    type Signature = Signatures;

    fn __check_auth(
        e: Env,
        payload: Hash<32>,      // used for provider auth
        signatures: Signatures, // provided by tx submitter in Authorization entry
        contexts: Vec<Context>, // require_auth_for_args
    ) -> Result<(), AuthError> {
        Self::handle_utxo_auth(&e, signatures.clone(), contexts)?;
        Self::require_provider(&e, payload, signatures)
    }
}

pub fn create_contract(e: &Env) -> (AuthModuleTestContractClient, Address) {
    let contract_id = e.register(AuthModuleTestContract, {});
    let contract = AuthModuleTestContractClient::new(e, &contract_id);
    // Initialize contract if needed
    (contract, contract_id)
}
