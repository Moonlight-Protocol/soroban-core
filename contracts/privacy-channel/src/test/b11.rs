#![cfg(test)]
//! B11: a withdrawal must not name the channel's own address. Such a withdrawal executes as a
//! net-zero `transfer(channel, channel, amount)` while `Supply` still decrements — UTXO claims
//! burn without any tokens leaving the contract.
extern crate std;

use crate::{test::test::create_contracts, transact::ChannelOperation};
use moonlight_errors::Error as ContractError;
use moonlight_helpers::testutils::snapshot::get_env_with_g_accounts;
use soroban_sdk::{vec, Error};

#[test]
fn test_b11_withdraw_to_channel_address_is_rejected() {
    let e = get_env_with_g_accounts();
    let (channel, _auth, _token, _admin) = create_contracts(&e);

    let op = ChannelOperation {
        spend: vec![&e],
        create: vec![&e],
        deposit: vec![&e],
        withdraw: vec![&e, (channel.address.clone(), 1_i128, vec![&e])],
    };

    assert_eq!(
        channel.try_transact(&op).err(),
        Some(Ok(Error::from_contract_error(
            ContractError::WithdrawToChannelAddress as u32
        )))
    );
}
