use moonlight_auth::core::{ProviderAuthorizable, UtxoAuthorizable};
use moonlight_errors::Error as MoonlightError;
use moonlight_helpers::parser::address_to_ed25519_pk_bytes;

use moonlight_primitives::Signatures;
use soroban_sdk::{
    auth::{Context, CustomAccountInterface},
    contract, contractevent, contractimpl,
    crypto::Hash,
    panic_with_error, Address, BytesN, Env, Vec,
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

// UC6: the council's quorum-authorized record that an asset channel was enabled or disabled.
// The contract holds NO channel/asset state — this event is the only on-chain artifact. The
// council-platform DB (sole authoritative writer) and every provider converge on it: `enabled`
// distinguishes enable/re-enable (true) from disable (false). `channel` is the privacy-channel
// contract id; `asset` is its token contract id (a channel is single-asset, so this is self-describing).
#[contractevent(data_format = "single-value")]
pub struct ChannelStateChanged {
    #[topic]
    pub channel: Address,
    #[topic]
    pub asset: Address,
    pub enabled: bool,
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

// Cap the acceptance window of a pending ownership transfer in-contract. Without this the
// window could run to the network max (~180 days), leaving a compromised pending key exploitable
// for months. 7 days is the hard ceiling; the standard operating value is 3 days (see `set_admin`).
const MAX_ACCEPTANCE_WINDOW: u32 = 7 * DAY_IN_LEDGERS;

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

    /// Initiate a two-step transfer of the admin (owner) role to `new_admin`.
    ///
    /// `live_until_ledger` is the ledger up to which `new_admin` may `accept_admin`. Pass
    /// `current_ledger + N`, where `N` is the acceptance window in ledgers. The standard window
    /// for a real ownership handover is **3 days** = `current_ledger + 3 * DAY_IN_LEDGERS`
    /// (51_840 ledgers); the sensible range is 24h (17_280) to 7d (120_960). There is deliberately
    /// no default — every call states its window explicitly.
    ///
    /// A non-zero window beyond the in-contract ceiling of 7 days (`MAX_ACCEPTANCE_WINDOW` =
    /// 120_960 ledgers past the current ledger) panics [`MoonlightError::AcceptanceWindowTooLong`].
    /// `live_until_ledger == 0` is exempt from the ceiling and cancels a pending transfer (the
    /// library requires the cancel call to name the current pending address).
    pub fn set_admin(e: &Env, new_admin: Address, live_until_ledger: u32) {
        if live_until_ledger != 0
            && live_until_ledger > e.ledger().sequence() + MAX_ACCEPTANCE_WINDOW
        {
            panic_with_error!(e, MoonlightError::AcceptanceWindowTooLong);
        }
        ownable::transfer_ownership(e, &new_admin, live_until_ledger);
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

    /// Registers a provider. Only an Ed25519 account address can be registered: the provider
    /// check in `require_provider` matches exclusively addresses derived from `SignerKey::
    /// Provider` Ed25519 keys, so any other address kind (e.g. a contract address) would be
    /// registered — and reported registered by `is_provider` — yet could never authorize
    /// anything (RV audit B7).
    ///
    /// ### Panics
    /// - Panics `NotEd25519AccountAddress` if `provider` is a contract address.
    /// - Panics if the provider is already registered.
    pub fn add_provider(e: &Env, provider: Address) {
        ownable::enforce_owner_auth(e);
        address_to_ed25519_pk_bytes(e, &provider);
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

// UC6: asset-lifecycle. Quorum-gated, event-only — the contract stores no channel/asset state;
// it only emits the quorum-authorized record that the council DB and providers converge on. The
// owner is the council quorum account, so `enforce_owner_auth` is the quorum gate (mirrors
// add_provider/remove_provider).
#[contractimpl]
impl ChannelAuthContract {
    /// Enable an asset `channel` for service. Also used to RE-ENABLE a previously disabled
    /// channel — both resume full service, so both emit `ChannelStateChanged { enabled: true }`.
    ///
    /// # Advisory lifecycle — event-only, no on-chain enforcement (RV audit B12)
    ///
    /// The channel lifecycle is **advisory**. This function only emits
    /// `ChannelStateChanged`, signalling the council's *intended* channel state; the
    /// contract intentionally stores no channel/asset state, and no on-chain code path
    /// reads the event. Enforcement lives provider-side: providers treat a disabled
    /// channel as withdraw-only (new deposits and sends rejected, withdrawals served).
    /// The sole on-chain way to stop a channel is a contract `upgrade`.
    ///
    /// An on-chain enabled-flag with `transact` gating (e.g. block new deposits while
    /// still allowing withdrawals) is deferred future work.
    pub fn enable_channel(e: &Env, channel: Address, asset: Address) {
        ownable::enforce_owner_auth(e);
        ChannelStateChanged {
            channel,
            asset,
            enabled: true,
        }
        .publish(e);
    }

    /// Disable an asset `channel`. The channel becomes withdraw-only (new deposits/sends rejected);
    /// that enforcement lives provider-side. Emits `ChannelStateChanged { enabled: false }`.
    ///
    /// # Advisory lifecycle — event-only, no on-chain enforcement (RV audit B12)
    ///
    /// The channel lifecycle is **advisory**. This function does NOT stop the channel
    /// on-chain: it only emits `ChannelStateChanged`, signalling the council's
    /// *intended* channel state. The contract intentionally stores no channel/asset
    /// state, and `transact` on the privacy channel remains fully functional after this
    /// call. Enforcement lives provider-side: providers treat a disabled channel as
    /// withdraw-only. The sole on-chain way to stop a channel is a contract `upgrade`.
    ///
    /// An on-chain enabled-flag with `transact` gating (e.g. block new deposits while
    /// still allowing withdrawals) is deferred future work.
    pub fn disable_channel(e: &Env, channel: Address, asset: Address) {
        ownable::enforce_owner_auth(e);
        ChannelStateChanged {
            channel,
            asset,
            enabled: false,
        }
        .publish(e);
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
