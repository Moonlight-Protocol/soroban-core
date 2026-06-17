use soroban_sdk::{
    testutils::{Address as _, Events, MockAuth, MockAuthInvoke},
    Address, Env, Event, IntoVal,
};

use super::tests::create_contract;
use crate::contract::ChannelAuthContractClient;
use crate::contract::{ChannelStateChanged, ContractInitialized, ProviderAdded, ProviderRemoved};

fn enable_channel_with_auth(
    client: &ChannelAuthContractClient,
    admin: &Address,
    channel: &Address,
    asset: &Address,
    e: &Env,
) {
    client
        .mock_auths(&[MockAuth {
            address: admin,
            invoke: &MockAuthInvoke {
                contract: &client.address,
                fn_name: "enable_channel",
                args: (channel, asset).into_val(e),
                sub_invokes: &[],
            },
        }])
        .enable_channel(channel, asset);
}

fn disable_channel_with_auth(
    client: &ChannelAuthContractClient,
    admin: &Address,
    channel: &Address,
    asset: &Address,
    e: &Env,
) {
    client
        .mock_auths(&[MockAuth {
            address: admin,
            invoke: &MockAuthInvoke {
                contract: &client.address,
                fn_name: "disable_channel",
                args: (channel, asset).into_val(e),
                sub_invokes: &[],
            },
        }])
        .disable_channel(channel, asset);
}

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

#[test]
fn test_enable_channel_emits_event() {
    let e = Env::default();
    let (client, admin) = create_contract(&e);
    let channel = Address::generate(&e);
    let asset = Address::generate(&e);

    enable_channel_with_auth(&client, &admin, &channel, &asset, &e);

    let events = e.events().all();
    let last = events.events().last().unwrap();
    assert_eq!(
        last,
        &ChannelStateChanged {
            channel,
            asset,
            enabled: true,
        }
        .to_xdr(&e, &client.address)
    );
}

#[test]
fn test_disable_channel_emits_event() {
    let e = Env::default();
    let (client, admin) = create_contract(&e);
    let channel = Address::generate(&e);
    let asset = Address::generate(&e);

    disable_channel_with_auth(&client, &admin, &channel, &asset, &e);

    let events = e.events().all();
    let last = events.events().last().unwrap();
    assert_eq!(
        last,
        &ChannelStateChanged {
            channel,
            asset,
            enabled: false,
        }
        .to_xdr(&e, &client.address)
    );
}

// Enable -> disable -> re-enable. Re-enable reuses enable_channel and emits the same
// `enabled: true` record, so providers and the council DB resume full service on it.
#[test]
fn test_channel_lifecycle_enable_disable_reenable() {
    let e = Env::default();
    let (client, admin) = create_contract(&e);
    let channel = Address::generate(&e);
    let asset = Address::generate(&e);

    enable_channel_with_auth(&client, &admin, &channel, &asset, &e);
    disable_channel_with_auth(&client, &admin, &channel, &asset, &e);
    enable_channel_with_auth(&client, &admin, &channel, &asset, &e);

    let events = e.events().all();
    let last = events.events().last().unwrap();
    assert_eq!(
        last,
        &ChannelStateChanged {
            channel,
            asset,
            enabled: true,
        }
        .to_xdr(&e, &client.address)
    );
}

// Quorum gate: a call NOT authorized by the owner (council quorum) must be rejected, and emit
// no event. Mirrors the add_provider non-owner rejection in tests.rs.
#[test]
fn test_enable_channel_requires_owner_auth() {
    let e = Env::default();
    let (client, _admin) = create_contract(&e);
    let not_owner = Address::generate(&e);
    let channel = Address::generate(&e);
    let asset = Address::generate(&e);

    let res = client
        .mock_auths(&[MockAuth {
            address: &not_owner,
            invoke: &MockAuthInvoke {
                contract: &client.address,
                fn_name: "enable_channel",
                args: (&channel, &asset).into_val(&e),
                sub_invokes: &[],
            },
        }])
        .try_enable_channel(&channel, &asset);

    assert!(res.is_err());
}

#[test]
fn test_disable_channel_requires_owner_auth() {
    let e = Env::default();
    let (client, _admin) = create_contract(&e);
    let not_owner = Address::generate(&e);
    let channel = Address::generate(&e);
    let asset = Address::generate(&e);

    let res = client
        .mock_auths(&[MockAuth {
            address: &not_owner,
            invoke: &MockAuthInvoke {
                contract: &client.address,
                fn_name: "disable_channel",
                args: (&channel, &asset).into_val(&e),
                sub_invokes: &[],
            },
        }])
        .try_disable_channel(&channel, &asset);

    assert!(res.is_err());
}
