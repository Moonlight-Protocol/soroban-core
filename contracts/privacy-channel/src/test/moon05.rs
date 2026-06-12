#![cfg(test)]
//! MOON-05: deposit/withdraw amounts must be strictly positive (checked in-contract, before any
//! signature verification, rather than relying on the asset SAC to reject them).
extern crate std;

use crate::{test::test::create_contracts, transact::ChannelOperation};
use moonlight_errors::Error as ContractError;
use moonlight_helpers::testutils::snapshot::get_env_with_g_accounts;
use soroban_sdk::{testutils::Address as _, vec, Address, Error};

fn assert_invalid_amount(res_err: Option<Result<Error, soroban_sdk::InvokeError>>) {
    assert_eq!(
        res_err,
        Some(Ok(Error::from_contract_error(
            ContractError::InvalidExternalAmount as u32
        )))
    );
}

#[test]
fn test_moon05_zero_withdraw_amount_is_rejected() {
    let e = get_env_with_g_accounts();
    let (channel, _auth, _token, _admin) = create_contracts(&e);
    let to = Address::generate(&e);

    let op = ChannelOperation {
        spend: vec![&e],
        create: vec![&e],
        deposit: vec![&e],
        withdraw: vec![&e, (to, 0_i128, vec![&e])],
    };

    assert_invalid_amount(channel.try_transact(&op).err());
}

#[test]
fn test_moon05_negative_deposit_amount_is_rejected() {
    let e = get_env_with_g_accounts();
    let (channel, _auth, _token, _admin) = create_contracts(&e);
    let from = Address::generate(&e);

    let op = ChannelOperation {
        spend: vec![&e],
        create: vec![&e],
        deposit: vec![&e, (from, -1_i128, vec![&e])],
        withdraw: vec![&e],
    };

    assert_invalid_amount(channel.try_transact(&op).err());
}
