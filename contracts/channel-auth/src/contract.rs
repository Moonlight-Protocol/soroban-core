use moonlight_auth::core::{Error as AuthError, ProviderAuthorizable, UtxoAuthorizable};

use moonlight_primitives::Signatures;
use soroban_sdk::{
    auth::{Context, CustomAccountInterface},
    contract, contractevent, contractimpl,
    crypto::Hash,
    Address, BytesN, Env, Vec,
};
use stellar_access::ownable;
use stellar_contract_utils::upgradeable;

#[contractevent(data_format = "single-value")]
pub struct ContractInitialized {
    #[topic]
    pub admin: Address,
}

#[contractevent(data_format = "single-value")]
pub struct ProviderAdded {
    #[topic]
    pub provider: Address,
}

#[contractevent(data_format = "single-value")]
pub struct ProviderRemoved {
    #[topic]
    pub provider: Address,
}

#[contract]
pub struct ChannelAuthContract;

impl UtxoAuthorizable for ChannelAuthContract {}

#[contractimpl]
impl ChannelAuthContract {
    pub fn __constructor(env: &Env, admin: &Address) {
        ownable::set_owner(env, admin);
        ContractInitialized {
            admin: admin.clone(),
        }
        .publish(env);
    }

    pub fn admin(e: &Env) -> Address {
        ownable::get_owner(e).unwrap()
    }

    pub fn set_admin(e: &Env, new_admin: Address) {
        ownable::transfer_ownership(e, &new_admin, e.ledger().max_live_until_ledger());
    }

    pub fn accept_admin(e: &Env) {
        ownable::accept_ownership(e);
    }

    pub fn upgrade(e: &Env, wasm_hash: BytesN<32>) {
        ownable::enforce_owner_auth(e);
        upgradeable::upgrade(e, &wasm_hash);
    }
}

impl ProviderAuthorizable for ChannelAuthContract {}

#[contractimpl]
impl ChannelAuthContract {
    pub fn is_provider(e: &Env, provider: Address) -> bool {
        <Self as ProviderAuthorizable>::is_provider(e, provider)
    }

    pub fn add_provider(e: &Env, provider: Address) {
        ownable::enforce_owner_auth(e);
        let addr = provider.clone();
        Self::register_provider(e, provider);
        ProviderAdded { provider: addr }.publish(e);
    }

    pub fn remove_provider(e: &Env, provider: Address) {
        ownable::enforce_owner_auth(e);
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
