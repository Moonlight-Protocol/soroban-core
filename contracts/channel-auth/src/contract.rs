use admin_sep::{Administratable, Upgradable};
use moonlight_auth::core::{Error as AuthError, ProviderAuthorizable, UtxoAuthorizable};

use moonlight_primitives::Signatures;
use soroban_sdk::{
    auth::{Context, CustomAccountInterface},
    contract, contractevent, contractimpl,
    crypto::Hash,
    Address, Env, Vec,
};

#[contractevent(data_format = "single-value")]
pub struct ContractInitialized {
    #[topic]
    admin: Address,
}

#[contractevent(data_format = "single-value")]
pub struct ProviderAdded {
    #[topic]
    provider: Address,
}

#[contractevent(data_format = "single-value")]
pub struct ProviderRemoved {
    #[topic]
    provider: Address,
}

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
        ContractInitialized {
            admin: admin.clone(),
        }
        .publish(env);
    }
}

#[contractimpl]
impl ProviderAuthorizable for ChannelAuthContract {}

#[contractimpl]
impl ChannelAuthContract {
    pub fn add_provider(e: &Env, provider: Address) {
        Self::require_admin(e);
        let addr = provider.clone();
        Self::register_provider(e, provider);
        ProviderAdded { provider: addr }.publish(e);
    }

    pub fn remove_provider(e: &Env, provider: Address) {
        Self::require_admin(e);
        let addr = provider.clone();
        Self::deregister_provider(e, provider);
        ProviderRemoved { provider: addr }.publish(e);
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
