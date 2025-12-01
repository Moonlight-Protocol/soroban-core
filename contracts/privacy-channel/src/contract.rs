use admin_sep::{Administratable, Upgradable};

use moonlight_utxo_core::core::UtxoHandlerTrait;
use soroban_sdk::{contract, contractimpl, Address, BytesN, Env, Vec};

use crate::{
    storage::{read_asset, read_supply, write_asset_unchecked},
    transact::{execute_external_operations, pre_process_channel_operation, ChannelOperation},
};

#[contract]
pub struct PrivacyChannelContract;

#[contractimpl]
impl Administratable for PrivacyChannelContract {}

#[contractimpl]
impl Upgradable for PrivacyChannelContract {}

#[contractimpl]
impl UtxoHandlerTrait for PrivacyChannelContract {}

#[contractimpl]
impl PrivacyChannelContract {
    pub fn __constructor(e: &Env, admin: Address, auth_contract: Address, asset: Address) {
        Self::set_admin(e, &admin);
        Self::require_admin(e);
        Self::set_auth(e, &auth_contract);
        write_asset_unchecked(e, asset);
    }

    pub fn asset(e: Env) -> Address {
        read_asset(&e)
    }

    pub fn supply(e: Env) -> i128 {
        read_supply(&e)
    }

    pub fn transact(e: Env, op: ChannelOperation) {
        let (utxo_op, total_deposit, total_withdraw) =
            pre_process_channel_operation(&e, op.clone());

        Self::process_bundle(&e, utxo_op.clone(), total_deposit, total_withdraw);

        execute_external_operations(&e, op.deposit, op.withdraw);
    }
}
