use soroban_sdk::{
    testutils::{Address as _, Events, MockAuth, MockAuthInvoke},
    vec, Address, Env, IntoVal, Symbol, Val, Vec,
};

use crate::contract::ChannelAuthContractClient;
use super::tests::create_contract;

fn add_provider_with_auth(client: &ChannelAuthContractClient, admin: &Address, provider: &Address, e: &Env) {
    client
        .mock_auths(&[MockAuth {
            address: admin,
            invoke: &MockAuthInvoke {
                contract: &client.address,
                fn_name: "add_provider",
                args: (provider,).into_val(e),
                sub_invokes: &[],
            },
        }])
        .add_provider(provider);
}

fn remove_provider_with_auth(client: &ChannelAuthContractClient, admin: &Address, provider: &Address, e: &Env) {
    client
        .mock_auths(&[MockAuth {
            address: admin,
            invoke: &MockAuthInvoke {
                contract: &client.address,
                fn_name: "remove_provider",
                args: (provider,).into_val(e),
                sub_invokes: &[],
            },
        }])
        .remove_provider(provider);
}

#[test]
fn test_constructor_emits_initialized_event() {
    let e = Env::default();
    let (client, admin) = create_contract(&e);

    let events = e.events().all();
    let expected_topics: Vec<Val> = vec![
        &e,
        Symbol::new(&e, "contract_initialized").into_val(&e),
        admin.into_val(&e),
    ];

    let last = events.last().unwrap();
    assert_eq!(last.0, client.address);
    assert_eq!(last.1, expected_topics);
}

#[test]
fn test_add_provider_emits_event() {
    let e = Env::default();
    let (client, admin) = create_contract(&e);
    let provider = Address::generate(&e);

    add_provider_with_auth(&client, &admin, &provider, &e);

    let events = e.events().all();
    let expected_topics: Vec<Val> = vec![
        &e,
        Symbol::new(&e, "provider_added").into_val(&e),
        provider.into_val(&e),
    ];

    let last = events.last().unwrap();
    assert_eq!(last.0, client.address);
    assert_eq!(last.1, expected_topics);
}

#[test]
fn test_remove_provider_emits_event() {
    let e = Env::default();
    let (client, admin) = create_contract(&e);
    let provider = Address::generate(&e);

    add_provider_with_auth(&client, &admin, &provider, &e);
    remove_provider_with_auth(&client, &admin, &provider, &e);

    let events = e.events().all();
    let expected_topics: Vec<Val> = vec![
        &e,
        Symbol::new(&e, "provider_removed").into_val(&e),
        provider.into_val(&e),
    ];

    let last = events.last().unwrap();
    assert_eq!(last.0, client.address);
    assert_eq!(last.1, expected_topics);
}

#[test]
fn test_provider_lifecycle_with_events() {
    let e = Env::default();
    let (client, admin) = create_contract(&e);
    let provider = Address::generate(&e);

    add_provider_with_auth(&client, &admin, &provider, &e);
    assert!(client.is_provider(&provider));

    remove_provider_with_auth(&client, &admin, &provider, &e);
    assert!(!client.is_provider(&provider));
}
