#![cfg(test)]

use moonlight_helpers::testutils::keys::{Ed25519Account, P256KeyPair};
use moonlight_primitives::Condition;
use soroban_sdk::{
    testutils::{Address as _, MockAuth, MockAuthInvoke},
    vec, Address, Env, IntoVal,
};

use crate::contract::{ChannelAuthContract, ChannelAuthContractArgs, ChannelAuthContractClient};
use moonlight_utxo_core::testutils::{
    contract::create_contract as create_utxo_contract, operation_bundle::UTXOOperationBuilder,
};

pub fn create_contract(e: &Env) -> (ChannelAuthContractClient, Address) {
    let admin = Address::generate(&e);
    let contract_id = e.register(
        ChannelAuthContract,
        ChannelAuthContractArgs::__constructor(&admin),
    );
    let contract = ChannelAuthContractClient::new(e, &contract_id);
    // Initialize contract if needed
    (contract, admin)
}

#[test]
fn test_auth_module() {
    let e = Env::default();

    let (auth_client, admin) = create_contract(&e);
    let (utxo_client, _) = create_utxo_contract(&e, auth_client.address.clone());

    assert_eq!(auth_client.admin(), admin.clone());
    assert_eq!(utxo_client.auth(), auth_client.address);
    let provider = Ed25519Account::generate(&e);

    let mock_auth = MockAuth {
        address: &admin,
        invoke: &MockAuthInvoke {
            contract: &auth_client.address,
            fn_name: "add_provider",
            args: (&provider.address,).into_val(&e),
            sub_invokes: &[],
        },
    };

    auth_client
        .mock_auths(&[mock_auth])
        .add_provider(&provider.address);

    assert_eq!(auth_client.is_provider(&provider.address), true);

    let utxo_a = P256KeyPair::generate(&e);
    let utxo_b = P256KeyPair::generate(&e);
    let utxo_c = P256KeyPair::generate(&e);
    let utxo_d = P256KeyPair::generate(&e);

    utxo_client.mint(&vec![
        &e,
        (utxo_a.public_key.clone(), 1000_i128),
        (utxo_b.public_key.clone(), 500_i128),
    ]);

    let mut op = UTXOOperationBuilder::generate(
        &e,
        utxo_client.address.clone(),
        auth_client.address.clone(),
    );

    op.add_create(utxo_c.public_key.clone(), 700_i128);
    op.add_create(utxo_d.public_key.clone(), 800_i128);

    op.add_spend(
        utxo_a.public_key.clone(),
        vec![&e, Condition::Create(utxo_c.public_key.clone(), 700_i128)],
    );
    op.add_spend(
        utxo_b.public_key.clone(),
        vec![&e, Condition::Create(utxo_d.public_key.clone(), 800_i128)],
    );

    let live_until_ledger = e.ledger().sequence() + 1;

    let signature_a = utxo_a.sign(&op.get_auth_hash_for_spend(
        &e,
        utxo_a.public_key.clone(),
        live_until_ledger.clone(),
    ));

    op.add_spend_signature(
        &e,
        utxo_a.public_key.clone(),
        signature_a,
        live_until_ledger,
    );

    let signature_b = utxo_b.sign(&op.get_auth_hash_for_spend(
        &e,
        utxo_b.public_key.clone(),
        live_until_ledger.clone(),
    ));
    op.add_spend_signature(
        &e,
        utxo_b.public_key.clone(),
        signature_b,
        live_until_ledger,
    );

    let nonce = 0;
    let signature_provider = provider.sign(
        &e,
        op.get_auth_entry_payload_hash_for_bundle(&e, nonce.clone(), live_until_ledger.clone()),
    );

    op.add_provider_signature(&e, provider.address, signature_provider, live_until_ledger);

    utxo_client
        .set_auths(&[op.get_auth_entry(&e, nonce, live_until_ledger)])
        .transact(&op.get_operation_bundle());

    assert_eq!(utxo_client.utxo_balance(&utxo_a.public_key), 0);
    assert_eq!(utxo_client.utxo_balance(&utxo_b.public_key), 0);
    assert_eq!(utxo_client.utxo_balance(&utxo_c.public_key), 700);
    assert_eq!(utxo_client.utxo_balance(&utxo_d.public_key), 800);
}
