use moonlight_helpers::testutils::keys::{Ed25519Account, P256KeyPair};
use moonlight_primitives::{Condition, Signature, SignerKey};
use soroban_sdk::{testutils::Ledger, vec, xdr, Env, Error, Map};

use crate::{core::verify_signature, testutils::contract::create_contract};
use moonlight_utxo_core::testutils::{
    contract::create_contract as create_utxo_contract, operation_bundle::UTXOOperationBuilder,
};

#[test]
fn test_auth_module() {
    let e = Env::default();

    let (auth_client, _) = create_contract(&e);
    let (utxo_client, _) = create_utxo_contract(&e, auth_client.address.clone());

    assert_eq!(utxo_client.auth(), auth_client.address);

    let provider = Ed25519Account::generate(&e);
    auth_client.add_provider(&provider.address);

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

#[test]
fn test_auth_module_errors() {
    let e: Env = Env::default();

    let (auth_client, _) = create_contract(&e);
    let (utxo_client, _) = create_utxo_contract(&e, auth_client.address.clone());

    assert_eq!(utxo_client.auth(), auth_client.address);

    let provider = Ed25519Account::generate(&e);
    auth_client.add_provider(&provider.address);

    assert_eq!(auth_client.is_provider(&provider.address), true);

    let utxo_a = P256KeyPair::generate(&e);
    let utxo_b = P256KeyPair::generate(&e);

    utxo_client.mint(&vec![&e, (utxo_a.public_key.clone(), 1000_i128)]);

    let mut op = UTXOOperationBuilder::generate(
        &e,
        utxo_client.address.clone(),
        auth_client.address.clone(),
    );

    op.add_create(utxo_b.public_key.clone(), 1000_i128);

    op.add_spend(
        utxo_a.public_key.clone(),
        vec![&e, Condition::Create(utxo_b.public_key.clone(), 1000_i128)],
    );

    e.ledger().set_sequence_number(10);
    let expired_live_until_ledger = 9;

    let signature_a = utxo_a.sign(&op.get_auth_hash_for_spend(
        &e,
        utxo_a.public_key.clone(),
        expired_live_until_ledger.clone(),
    ));

    op.add_spend_signature(
        &e,
        utxo_a.public_key.clone(),
        signature_a,
        expired_live_until_ledger,
    );

    let nonce = 0;
    let signature_provider = provider.sign(
        &e,
        op.get_auth_entry_payload_hash_for_bundle(
            &e,
            nonce.clone(),
            expired_live_until_ledger.clone(),
        ),
    );

    op.add_provider_signature(
        &e,
        provider.address,
        signature_provider,
        expired_live_until_ledger,
    );

    let expected_expired_error = utxo_client
        .set_auths(&[op.get_auth_entry(&e, nonce, expired_live_until_ledger)])
        .try_transact(&op.get_operation_bundle());

    // assert_eq!(
    //     expected_expired_error.err(),
    //     Some(Ok(Error::from_contract_error(
    //         ContractError::SignatureExpired as u32
    //     )))
    // );
    //
    // todo: check how to validate inner errors as the
    // auth errors surface with context/invalid action errors
    assert_eq!(
        expected_expired_error.err(),
        Some(Ok(Error::from_type_and_code(
            xdr::ScErrorType::Context,
            xdr::ScErrorCode::InvalidAction
        )))
    );
}

#[test]
fn test_ed25519_signatures() {
    let e: Env = Env::default();

    let g_account_keypair = Ed25519Account::generate(&e);

    let payload = b"Hello, world!";

    let hash = e
        .crypto()
        .sha256(&soroban_sdk::Bytes::from_array(&e, payload));
    let signature = g_account_keypair.sign(&e, hash.clone());

    let mut sign_map = Map::new(&e);
    sign_map.set(
        SignerKey::Ed25519(g_account_keypair.public_key.clone()),
        (Signature::Ed25519(signature.clone()), u32::MAX),
    );

    for signer in sign_map.keys().iter() {
        let (signature, _) = sign_map.get(signer.clone()).unwrap(); // or handle Option

        match signer.clone() {
            SignerKey::Ed25519(pk) => {
                assert_eq!(&g_account_keypair.public_key, &pk);
            }
            _ => panic!("Unexpected signer key type"),
        }

        verify_signature(&e, &signer, &signature, &hash).unwrap();
    }
}
