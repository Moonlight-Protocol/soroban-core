use admin_sep::{Administratable, Upgradable};
use moonlight_auth::{Error as AuthError, ProviderAuthorizable, Signatures, UtxoAuthorizable};

use soroban_sdk::{
    auth::{Context, CustomAccountInterface},
    contract, contractimpl,
    crypto::Hash,
    Address, Env, Vec,
};

#[contract]
pub struct UTXOAuthContract;

#[contractimpl]
impl Administratable for UTXOAuthContract {}

#[contractimpl]
impl Upgradable for UTXOAuthContract {}

#[contractimpl]
impl UtxoAuthorizable for UTXOAuthContract {}

#[contractimpl]
impl UTXOAuthContract {
    pub fn __constructor(env: &Env, admin: &Address) {
        Self::set_admin(env, admin);
    }
}

#[contractimpl]
impl ProviderAuthorizable for UTXOAuthContract {}

#[contractimpl]
impl UTXOAuthContract {
    pub fn add_provider(e: &Env, provider: Address) {
        Self::require_admin(e);
        Self::register_provider(e, provider);
    }

    pub fn remove_provider(e: &Env, provider: Address) {
        Self::require_admin(e);
        Self::deregister_provider(e, provider);
    }
}

#[contractimpl]
impl CustomAccountInterface for UTXOAuthContract {
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
