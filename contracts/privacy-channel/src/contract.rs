use moonlight_errors::Error;
use moonlight_utxo_core::core::UtxoHandlerTrait;
use soroban_sdk::{
    contract, contractevent, contractimpl, panic_with_error, symbol_short, Address, BytesN, Env,
    Symbol, Vec,
};
use stellar_access::ownable;
use stellar_contract_utils::upgradeable;

// MOON-09: dedicated upgrade event for the governance audit trail.
#[contractevent(data_format = "single-value")]
pub struct Upgraded {
    #[topic]
    pub wasm_hash: BytesN<32>,
}

use crate::{
    storage::{read_asset, read_supply, write_asset_unchecked},
    transact::{execute_external_operations, pre_process_channel_operation, ChannelOperation},
};

#[contract]
pub struct PrivacyChannelContract;

impl UtxoHandlerTrait for PrivacyChannelContract {}

// MOON-02: instance-storage holds the asset/auth bindings, supply, and owner; bump its TTL on
// every mutating entrypoint so the contract instance cannot archive out from under live channels.
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

// MOON-06: reentrancy guard. `transact` makes external SAC calls (deposit pull / withdraw push);
// a non-standard or malicious asset could call back into `transact`. The guard is a transient flag
// that is set on entry and cleared on exit; a re-entrant call observes it and reverts. (On revert
// the transient write is rolled back, so the flag never persists across transactions.)
const REENTRANCY_GUARD: Symbol = symbol_short!("RGUARD");

fn enter_reentrancy_guard(e: &Env) {
    if e.storage().temporary().has(&REENTRANCY_GUARD) {
        panic_with_error!(e, Error::ReentrantCall);
    }
    e.storage().temporary().set(&REENTRANCY_GUARD, &true);
}

fn exit_reentrancy_guard(e: &Env) {
    e.storage().temporary().remove(&REENTRANCY_GUARD);
}

#[contractimpl]
impl PrivacyChannelContract {
    pub fn __constructor(e: &Env, admin: Address, auth_contract: Address, asset: Address) {
        ownable::set_owner(e, &admin);
        ownable::enforce_owner_auth(e);
        <Self as UtxoHandlerTrait>::set_auth(e, &auth_contract);
        write_asset_unchecked(e, asset);
        bump_instance_ttl(e);
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
    /// 120_960 ledgers past the current ledger) panics [`Error::AcceptanceWindowTooLong`].
    /// `live_until_ledger == 0` is exempt from the ceiling and cancels a pending transfer (the
    /// library requires the cancel call to name the current pending address).
    pub fn set_admin(e: &Env, new_admin: Address, live_until_ledger: u32) {
        if live_until_ledger != 0
            && live_until_ledger > e.ledger().sequence() + MAX_ACCEPTANCE_WINDOW
        {
            panic_with_error!(e, Error::AcceptanceWindowTooLong);
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

    pub fn auth(e: &Env) -> Address {
        <Self as UtxoHandlerTrait>::auth(e)
    }

    pub fn utxo_balance(e: &Env, utxo: BytesN<65>) -> i128 {
        <Self as UtxoHandlerTrait>::utxo_balance(e, utxo)
    }

    pub fn utxo_balances(e: &Env, utxos: Vec<BytesN<65>>) -> Vec<i128> {
        <Self as UtxoHandlerTrait>::utxo_balances(e, utxos)
    }

    pub fn asset(e: Env) -> Address {
        read_asset(&e)
    }

    pub fn supply(e: Env) -> i128 {
        read_supply(&e)
    }

    pub fn transact(e: Env, op: ChannelOperation) {
        bump_instance_ttl(&e);
        enter_reentrancy_guard(&e);

        let (utxo_op, total_deposit, total_withdraw) =
            pre_process_channel_operation(&e, op.clone());

        Self::process_bundle(&e, utxo_op.clone(), total_deposit, total_withdraw);

        execute_external_operations(&e, op.deposit, op.withdraw);

        exit_reentrancy_guard(&e);
    }
}
