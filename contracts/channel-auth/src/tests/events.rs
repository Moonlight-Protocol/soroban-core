use soroban_sdk::{
    testutils::{Address as _, Events, MockAuth, MockAuthInvoke},
    Address, Env, Event, IntoVal,
};

use super::tests::create_contract;
use crate::contract::ChannelAuthContractClient;
use crate::contract::{ContractInitialized, ProviderAdded, ProviderRemoved};

fn add_provider_with_auth(
    client: &ChannelAuthContractClient,
    admin: &Address,
    provider: &Address,
    e: &Env,
) {
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

fn remove_provider_with_auth(
    client: &ChannelAuthContractClient,
    admin: &Address,
    provider: &Address,
    e: &Env,
) {
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
    let last = events.events().last().unwrap();
    assert_eq!(
        last,
        &ContractInitialized { admin }.to_xdr(&e, &client.address)
    );
}

#[test]
fn test_add_provider_emits_event() {
    let e = Env::default();
    let (client, admin) = create_contract(&e);
    let provider = Address::generate(&e);

    add_provider_with_auth(&client, &admin, &provider, &e);

    let events = e.events().all();
    let last = events.events().last().unwrap();
    assert_eq!(
        last,
        &ProviderAdded { provider }.to_xdr(&e, &client.address)
    );
}

#[test]
fn test_remove_provider_emits_event() {
    let e = Env::default();
    let (client, admin) = create_contract(&e);
    let provider = Address::generate(&e);

    add_provider_with_auth(&client, &admin, &provider, &e);
    remove_provider_with_auth(&client, &admin, &provider, &e);

    let events = e.events().all();
    let last = events.events().last().unwrap();
    assert_eq!(
        last,
        &ProviderRemoved { provider }.to_xdr(&e, &client.address)
    );
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
