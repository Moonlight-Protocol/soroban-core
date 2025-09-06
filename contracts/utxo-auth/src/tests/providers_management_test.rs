#![cfg(test)]
use soroban_sdk::{
    testutils::Address as TestAddress,
    token::{StellarAssetClient, TokenClient},
    Address, Env,
};

use crate::{contract::PrivacyPoolContractClient, tests::helpers::create_contracts};

#[test]
fn test_manage_provider_success() {
    let e = Env::default();
    e.mock_all_auths();

    let (_admin, pool, _asset_client, _token_client): (
        Address,
        PrivacyPoolContractClient,
        StellarAssetClient,
        TokenClient,
    ) = create_contracts(&e);

    let provider_a = <soroban_sdk::Address as TestAddress>::generate(&e);
    let provider_b = <soroban_sdk::Address as TestAddress>::generate(&e);

    assert_eq!(pool.is_provider(&provider_a), false,);
    assert_eq!(pool.is_provider(&provider_b), false,);

    pool.register_provider(&provider_a);

    assert_eq!(pool.is_provider(&provider_a), true,);
    assert_eq!(pool.is_provider(&provider_b), false,);

    pool.register_provider(&provider_b);

    assert_eq!(pool.is_provider(&provider_a), true,);
    assert_eq!(pool.is_provider(&provider_b), true,);

    pool.deregister_provider(&provider_a);

    assert_eq!(pool.is_provider(&provider_a), false,);
    assert_eq!(pool.is_provider(&provider_b), true,);

    pool.deregister_provider(&provider_b);

    assert_eq!(pool.is_provider(&provider_a), false,);
    assert_eq!(pool.is_provider(&provider_b), false,);
}

#[test]
#[should_panic]
fn test_register_provider_without_admin_fail() {
    let e = Env::default();
    e.mock_all_auths();

    let (_admin, pool, _asset_client, _token_client): (
        Address,
        PrivacyPoolContractClient,
        StellarAssetClient,
        TokenClient,
    ) = create_contracts(&e);

    let provider = <soroban_sdk::Address as TestAddress>::generate(&e);

    assert_eq!(pool.is_provider(&provider), false,);

    e.set_auths(&[]);
    pool.register_provider(&provider);
}

#[test]
#[should_panic]
fn test_deregister_provider_without_admin_fail() {
    let e = Env::default();
    e.mock_all_auths();

    let (_admin, pool, _asset_client, _token_client): (
        Address,
        PrivacyPoolContractClient,
        StellarAssetClient,
        TokenClient,
    ) = create_contracts(&e);

    let provider = <soroban_sdk::Address as TestAddress>::generate(&e);

    assert_eq!(pool.is_provider(&provider), false,);

    pool.register_provider(&provider);

    e.set_auths(&[]);
    pool.deregister_provider(&provider);
}
