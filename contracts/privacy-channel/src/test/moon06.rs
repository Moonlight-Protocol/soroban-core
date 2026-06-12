#![cfg(test)]
//! MOON-06: a malicious/non-standard asset SAC must not be able to re-enter `transact` via the
//! deposit/withdraw transfer callback. The reentrancy guard rejects the nested call.
extern crate std;

use crate::{
    contract::{PrivacyChannelContract, PrivacyChannelContractArgs, PrivacyChannelContractClient},
    transact::ChannelOperation,
};
use channel_auth_contract::contract::{ChannelAuthContract, ChannelAuthContractArgs};
use moonlight_helpers::testutils::{keys::P256KeyPair, snapshot::get_env_with_g_accounts};
use moonlight_primitives::Condition;
use soroban_sdk::{
    contract, contractimpl, symbol_short, testutils::Address as _, vec, Address, Env, MuxedAddress,
};

/// A malicious "asset" whose `transfer` re-enters the channel's `transact`.
#[contract]
struct ReentrantAsset;

#[contractimpl]
impl ReentrantAsset {
    pub fn set_channel(e: Env, channel: Address) {
        e.storage().instance().set(&symbol_short!("CH"), &channel);
    }

    // Mirrors the SEP-41 token `transfer` signature so the channel's `TokenClient` resolves to it.
    pub fn transfer(e: Env, _from: Address, _to: MuxedAddress, _amount: i128) {
        let channel: Address = e.storage().instance().get(&symbol_short!("CH")).unwrap();
        let empty = ChannelOperation {
            spend: vec![&e],
            create: vec![&e],
            deposit: vec![&e],
            withdraw: vec![&e],
        };
        PrivacyChannelContractClient::new(&e, &channel).transact(&empty);
    }
}

#[test]
fn test_moon06_reentrant_asset_call_is_rejected() {
    let e = get_env_with_g_accounts();
    e.mock_all_auths();

    let admin = Address::generate(&e);
    let auth_id = e.register(
        ChannelAuthContract,
        ChannelAuthContractArgs::__constructor(&admin),
    );
    let asset_id = e.register(ReentrantAsset, ());
    let channel_id = e.register(
        PrivacyChannelContract,
        PrivacyChannelContractArgs::__constructor(&admin, &auth_id, &asset_id),
    );
    let channel = PrivacyChannelContractClient::new(&e, &channel_id);

    ReentrantAssetClient::new(&e, &asset_id).set_channel(&channel_id);

    // A deposit triggers asset.transfer(...), which re-enters channel.transact.
    let depositor = Address::generate(&e);
    let utxo = P256KeyPair::generate(&e);
    let op = ChannelOperation {
        spend: vec![&e],
        create: vec![&e, (utxo.public_key.clone(), 100_i128)],
        deposit: vec![
            &e,
            (
                depositor,
                100_i128,
                vec![&e, Condition::Create(utxo.public_key.clone(), 100_i128)],
            ),
        ],
        withdraw: vec![&e],
    };

    let res = channel.try_transact(&op);

    // The re-entrant call trips the guard; the whole transaction reverts. (The inner ReentrantCall
    // surfaces through the asset/auth boundary as the host's Context/InvalidAction wrap, as the
    // codebase documents for cross-boundary contract errors.) Without the guard the inner empty
    // transact would be a no-op and the deposit would COMPLETE — so the revert + unchanged state
    // below is attributable to the guard.
    assert!(res.is_err());
    assert_eq!(channel.supply(), 0); // deposit did not go through
    assert_eq!(channel.utxo_balance(&utxo.public_key), -1); // UTXO not created
}
