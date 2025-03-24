#![cfg(test)]

use crate::tests::helpers::create_contract;
use soroban_sdk::{testutils::Address, Env};

#[test]
fn test_register_provider_success() {
    let e = Env::default();

    let contract = create_contract(&e);
    let provider = <soroban_sdk::Address as Address>::generate(&e);

    assert_eq!(contract.is_provider(&provider), false,);

    contract.register_provider(&provider);

    assert_eq!(contract.is_provider(&provider), true,);
}

#[test]
fn test_deregister_provider_success() {
    let e = Env::default();

    let contract = create_contract(&e);
    let provider = <soroban_sdk::Address as Address>::generate(&e);

    contract.register_provider(&provider);
    assert_eq!(contract.is_provider(&provider), true,);

    contract.deregister_provider(&provider);
    assert_eq!(contract.is_provider(&provider), false,);
}

#[test]
fn test_only_provider_success() {
    let e = Env::default();
    e.mock_all_auths();

    let contract = create_contract(&e);
    let provider = <soroban_sdk::Address as Address>::generate(&e);

    contract.register_provider(&provider);

    assert_eq!(contract.only_provider(&provider), true,);
}

#[test]
#[should_panic]
fn test_only_provider_without_auth_failure() {
    let e = Env::default();

    let contract = create_contract(&e);
    let provider = <soroban_sdk::Address as Address>::generate(&e);

    contract.register_provider(&provider);

    contract.only_provider(&provider);
}

#[test]
#[should_panic]
fn test_only_provider_with_auth_but_not_provider_failure() {
    let e = Env::default();
    e.mock_all_auths();

    let contract = create_contract(&e);
    let provider = <soroban_sdk::Address as Address>::generate(&e);

    contract.only_provider(&provider);
}
