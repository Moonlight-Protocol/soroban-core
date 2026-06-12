use moonlight_auth::core::{ProviderAuthorizable, UtxoAuthorizable};
use moonlight_errors::Error as MoonlightError;

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

// MOON-09: emit a dedicated event on upgrade so the governance audit trail does not rely solely on
// the raw Stellar transaction record.
#[contractevent(data_format = "single-value")]
pub struct Upgraded {
    #[topic]
    pub wasm_hash: BytesN<32>,
}

#[contract]
pub struct ChannelAuthContract;

impl UtxoAuthorizable for ChannelAuthContract {}

// MOON-02: instance-storage holds the provider set and owner; bump its TTL on construction and on
// every auth check (which happens on every governed bundle) so it cannot archive while in use.
const DAY_IN_LEDGERS: u32 = 17_280;
const INSTANCE_BUMP_AMOUNT: u32 = 7 * DAY_IN_LEDGERS;
const INSTANCE_LIFETIME_THRESHOLD: u32 = INSTANCE_BUMP_AMOUNT - DAY_IN_LEDGERS;

fn bump_instance_ttl(e: &Env) {
    e.storage()
        .instance()
        .extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);
}

#[contractimpl]
impl ChannelAuthContract {
    pub fn __constructor(env: &Env, admin: &Address) {
        ownable::set_owner(env, admin);
        bump_instance_ttl(env);
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
        Upgraded {
            wasm_hash: wasm_hash.clone(),
        }
        .publish(e);
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
    type Error = MoonlightError;
    type Signature = Signatures;

    fn __check_auth(
        e: Env,
        payload: Hash<32>,      // used for provider auth
        signatures: Signatures, // provided by tx submitter in Authorization entry
        contexts: Vec<Context>, // require_auth_for_args
    ) -> Result<(), MoonlightError> {
        bump_instance_ttl(&e);
        Self::require_provider(&e, payload, signatures.clone())?;
        Self::handle_utxo_auth(&e, signatures.clone(), contexts)
    }
}
