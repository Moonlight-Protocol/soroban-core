#![cfg(test)]

use moonlight_auth::Condition;
use moonlight_helpers::testutils::keys::{Ed25519Account, P256KeyPair};
use soroban_sdk::{vec, Address, Env};
extern crate std;

use moonlight_utxo_core::{tests::helpers::TestContractClient, utxo_bundle::UTXOOperationBuilder};

use crate::{contract::UTXOAuthContractClient, tests::helpers::create_contracts};

#[test]
fn test_minimal_auth_bundle_success() {
    let e = Env::default();

    let (admin, auth, utxo_mock): (Address, UTXOAuthContractClient, TestContractClient) =
        create_contracts(&e);

    let provider = Ed25519Account::generate(&e);

    assert_eq!(auth.admin(), admin);
    assert_eq!(auth.is_provider(&provider.address), false);

    auth.mock_all_auths().add_provider(&provider.address);
    assert_eq!(auth.is_provider(&provider.address), true);

    assert_eq!(utxo_mock.utxo_auth(), auth.address);

    let utxo_a = P256KeyPair::generate(&e);
    let utxo_b = P256KeyPair::generate(&e);

    assert_eq!(utxo_mock.utxo_balance(&utxo_a.public_key.clone()), -1_i128);

    assert_eq!(utxo_mock.utxo_balance(&utxo_b.public_key.clone()), -1_i128);

    utxo_mock.mint_unchecked(&vec![&e, (utxo_a.public_key.clone(), 250_i128)]);

    assert_eq!(utxo_mock.utxo_balance(&utxo_a.public_key.clone()), 250_i128);

    let mut bundle =
        UTXOOperationBuilder::generate(&e, utxo_mock.address.clone(), auth.address.clone());

    bundle.add_spend(
        utxo_a.public_key.clone(),
        vec![&e, Condition::Create(utxo_b.public_key.clone(), 250)],
    );

    bundle.add_create(utxo_b.public_key.clone(), 250);

    let nonce = 0;
    let signature_expiration_ledger = e.ledger().sequence() + 100;

    let a_signature = utxo_a.sign(&bundle.get_auth_hash_for_spend(
        &e,
        utxo_a.public_key.clone(),
        signature_expiration_ledger,
    ));

    bundle.add_spend_signature(
        &e,
        utxo_a.public_key.clone(),
        a_signature,
        signature_expiration_ledger,
    );

    let bundle_payload_hash =
        bundle.get_auth_entry_payload_hash_for_bundle(&e, nonce, signature_expiration_ledger);

    let signed_payload = provider.sign(&e, bundle_payload_hash);

    bundle.add_provider_signature(&e, provider.address.clone(), signed_payload);

    let result = utxo_mock
        .set_auths(&[bundle.get_auth_entry(&e, nonce, signature_expiration_ledger)])
        .transact(&bundle.get_operation_bundle().clone());

    assert_eq!(result, 0_i128);

    assert_eq!(utxo_mock.utxo_balance(&utxo_a.public_key.clone()), 0_i128);
    assert_eq!(utxo_mock.utxo_balance(&utxo_b.public_key.clone()), 250_i128);
}

#[test]
fn test_multiple_auth_bundle_success() {
    let e = Env::default();

    let (admin, auth, utxo_mock): (Address, UTXOAuthContractClient, TestContractClient) =
        create_contracts(&e);

    assert_eq!(utxo_mock.utxo_auth(), auth.address);

    let provider = Ed25519Account::generate(&e);

    assert_eq!(auth.admin(), admin);
    assert_eq!(auth.is_provider(&provider.address), false);

    auth.mock_all_auths().add_provider(&provider.address);
    assert_eq!(auth.is_provider(&provider.address), true);

    let utxo_a = P256KeyPair::generate(&e);
    let utxo_b = P256KeyPair::generate(&e);
    let utxo_c = P256KeyPair::generate(&e);
    let utxo_d = P256KeyPair::generate(&e);
    let utxo_e = P256KeyPair::generate(&e);
    let utxo_f = P256KeyPair::generate(&e);

    assert_eq!(utxo_mock.utxo_balance(&utxo_a.public_key.clone()), -1_i128);

    assert_eq!(utxo_mock.utxo_balance(&utxo_b.public_key.clone()), -1_i128);
    assert_eq!(utxo_mock.utxo_balance(&utxo_c.public_key.clone()), -1_i128);

    assert_eq!(utxo_mock.utxo_balance(&utxo_d.public_key.clone()), -1_i128);

    assert_eq!(utxo_mock.utxo_balance(&utxo_e.public_key.clone()), -1_i128);
    assert_eq!(utxo_mock.utxo_balance(&utxo_f.public_key.clone()), -1_i128);

    utxo_mock.mint_unchecked(&vec![
        &e,
        (utxo_a.public_key.clone(), 250_i128),
        (utxo_d.public_key.clone(), 50_i128),
        (utxo_e.public_key.clone(), 60_i128),
    ]);

    assert_eq!(utxo_mock.utxo_balance(&utxo_a.public_key.clone()), 250_i128);

    assert_eq!(utxo_mock.utxo_balance(&utxo_d.public_key.clone()), 50_i128);
    assert_eq!(utxo_mock.utxo_balance(&utxo_e.public_key.clone()), 60_i128);

    let mut bundle =
        UTXOOperationBuilder::generate(&e, utxo_mock.address.clone(), auth.address.clone());

    bundle.add_spend(
        utxo_a.public_key.clone(),
        vec![
            &e,
            Condition::Create(utxo_b.public_key.clone(), 250),
            Condition::Create(utxo_c.public_key.clone(), 10),
        ],
    );

    bundle.add_spend(
        utxo_e.public_key.clone(),
        vec![&e, Condition::Create(utxo_b.public_key.clone(), 250)],
    );

    bundle.add_spend(
        utxo_d.public_key.clone(),
        vec![&e, Condition::Create(utxo_f.public_key.clone(), 100)],
    );

    bundle.add_create(utxo_b.public_key.clone(), 250);

    bundle.add_create(utxo_c.public_key.clone(), 10);
    bundle.add_create(utxo_f.public_key.clone(), 100);

    let signature_expiration_ledger = e.ledger().sequence() + 100;

    let signature_a = utxo_a.sign(&bundle.get_auth_hash_for_spend(
        &e,
        utxo_a.public_key.clone(),
        signature_expiration_ledger,
    ));

    bundle.add_spend_signature(
        &e,
        utxo_a.public_key.clone(),
        signature_a,
        signature_expiration_ledger,
    );

    let signature_d = utxo_d.sign(&bundle.get_auth_hash_for_spend(
        &e,
        utxo_d.public_key.clone(),
        signature_expiration_ledger,
    ));
    bundle.add_spend_signature(
        &e,
        utxo_d.public_key.clone(),
        signature_d,
        signature_expiration_ledger,
    );

    let signature_e = utxo_e.sign(&bundle.get_auth_hash_for_spend(
        &e,
        utxo_e.public_key.clone(),
        signature_expiration_ledger,
    ));

    bundle.add_spend_signature(
        &e,
        utxo_e.public_key.clone(),
        signature_e,
        signature_expiration_ledger,
    );

    let nonce = 0;

    let provider_signature = provider.sign(
        &e,
        bundle.get_auth_entry_payload_hash_for_bundle(&e, nonce, signature_expiration_ledger),
    );
    bundle.add_provider_signature(&e, provider.address.clone(), provider_signature);

    let result = utxo_mock
        .set_auths(&[bundle.get_auth_entry(&e, nonce, signature_expiration_ledger)])
        .transact(&bundle.get_operation_bundle().clone());

    assert_eq!(result, 0_i128);

    assert_eq!(utxo_mock.utxo_balance(&utxo_a.public_key.clone()), 0_i128);

    assert_eq!(utxo_mock.utxo_balance(&utxo_d.public_key.clone()), 0_i128);
    assert_eq!(utxo_mock.utxo_balance(&utxo_e.public_key.clone()), 0_i128);

    assert_eq!(utxo_mock.utxo_balance(&utxo_b.public_key.clone()), 250_i128);
    assert_eq!(utxo_mock.utxo_balance(&utxo_c.public_key.clone()), 10_i128);
    assert_eq!(utxo_mock.utxo_balance(&utxo_f.public_key.clone()), 100_i128);
}
