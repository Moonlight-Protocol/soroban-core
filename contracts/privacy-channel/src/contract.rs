use moonlight_utxo_core::core::UtxoHandlerTrait;
use soroban_sdk::{contract, contractimpl, Address, BytesN, Env, Vec};
use stellar_access::ownable;
use stellar_contract_utils::upgradeable;

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

fn bump_instance_ttl(e: &Env) {
    e.storage()
        .instance()
        .extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);
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

        let (utxo_op, total_deposit, total_withdraw) =
            pre_process_channel_operation(&e, op.clone());

        Self::process_bundle(&e, utxo_op.clone(), total_deposit, total_withdraw);

        execute_external_operations(&e, op.deposit, op.withdraw);
    }
}
