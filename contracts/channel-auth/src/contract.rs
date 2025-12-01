use admin_sep::{Administratable, Upgradable};
use moonlight_auth::core::{Error as AuthError, ProviderAuthorizable, UtxoAuthorizable};

use moonlight_primitives::Signatures;
use soroban_sdk::{
    auth::{Context, CustomAccountInterface},
    contract, contractimpl,
    crypto::Hash,
    Address, Env, Vec,
};

#[contract]
pub struct ChannelAuthContract;

#[contractimpl]
impl Administratable for ChannelAuthContract {}

#[contractimpl]
impl Upgradable for ChannelAuthContract {}

#[contractimpl]
impl UtxoAuthorizable for ChannelAuthContract {}

#[contractimpl]
impl ChannelAuthContract {
    pub fn __constructor(env: &Env, admin: &Address) {
        Self::set_admin(env, admin);
    }
}

#[contractimpl]
impl ProviderAuthorizable for ChannelAuthContract {}

#[contractimpl]
impl ChannelAuthContract {
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
impl CustomAccountInterface for ChannelAuthContract {
    type Error = AuthError;
    type Signature = Signatures;

    fn __check_auth(
        e: Env,
        payload: Hash<32>,      // used for provider auth
        signatures: Signatures, // provided by tx submitter in Authorization entry
        contexts: Vec<Context>, // require_auth_for_args
    ) -> Result<(), AuthError> {
        Self::require_provider(&e, payload, signatures.clone())?;
        Self::handle_utxo_auth(&e, signatures.clone(), contexts)
    }
}
