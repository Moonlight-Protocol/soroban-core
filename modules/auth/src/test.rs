use moonlight_errors::Error as MoonlightError;
use moonlight_helpers::testutils::keys::{Ed25519Account, P256KeyPair};
use moonlight_primitives::{Condition, Signature, Signatures, SignerKey};
use soroban_sdk::{
    auth::{Context, ContractContext},
    testutils::{Address as _, Ledger},
    vec, xdr, Address, Env, Error, Map, Symbol,
};

use crate::{
    core::{verify_signature, UtxoAuthorizable},
    testutils::contract::{create_contract, AuthModuleTestContract},
};
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

/// MOON-03: an empty-args context must not short-circuit the whole `handle_utxo_auth` check.
/// With the empty context placed FIRST, the following spend-bearing context must still be
/// evaluated (and here fail `MissingSignature`, since no signature is supplied). Pre-fix the
/// `return Ok(())` skipped it entirely and the call wrongly succeeded.
#[test]
fn test_handle_utxo_auth_empty_context_does_not_skip_later_spend_context() {
    let e = Env::default();
    let (auth_client, _) = create_contract(&e);
    let channel = Address::generate(&e);
    let utxo = P256KeyPair::generate(&e);

    // A spend context that DOES require a P256 signature for `utxo`.
    let mut builder =
        UTXOOperationBuilder::generate(&e, channel.clone(), auth_client.address.clone());
    builder.add_spend(
        utxo.public_key.clone(),
        vec![&e, Condition::Create(utxo.public_key.clone(), 100_i128)],
    );
    let spend_args = builder.get_contract_auth_args(&e);

    let empty_ctx = Context::Contract(ContractContext {
        contract: channel.clone(),
        fn_name: Symbol::new(&e, "noop"),
        args: vec![&e], // empty args => no requirements for this context
    });
    let spend_ctx = Context::Contract(ContractContext {
        contract: channel.clone(),
        fn_name: Symbol::new(&e, "transact"),
        args: spend_args,
    });

    // No signatures supplied at all.
    let signatures = Signatures(Map::new(&e));

    let result = e.as_contract(&auth_client.address, || {
        <AuthModuleTestContract as UtxoAuthorizable>::handle_utxo_auth(
            &e,
            signatures.clone(),
            vec![&e, empty_ctx.clone(), spend_ctx.clone()],
        )
    });

    assert_eq!(result, Err(MoonlightError::MissingSignature));
}

/// MOON-04: lock the panic-on-failure semantic the verifier wrappers depend on. An invalid P256
/// signature (verified against a different payload than it signed) must panic / trap.
#[test]
#[should_panic]
fn signature_verification_panics_on_invalid_p256_signature() {
    let e = Env::default();
    let kp = P256KeyPair::generate(&e);
    let signed = e
        .crypto()
        .sha256(&soroban_sdk::Bytes::from_array(&e, b"the-signed-message"));
    let other = e
        .crypto()
        .sha256(&soroban_sdk::Bytes::from_array(&e, b"a-different-message"));
    let sig = kp.sign(&signed);
    let signature = Signature::P256(soroban_sdk::BytesN::<64>::from_array(&e, &sig));
    let signer = SignerKey::P256(kp.public_key.clone());

    verify_signature(&e, &signer, &signature, &other).unwrap();
}

/// MOON-04: same lock for Ed25519.
#[test]
#[should_panic]
fn signature_verification_panics_on_invalid_ed25519_signature() {
    let e = Env::default();
    let acc = Ed25519Account::generate(&e);
    let signed = e
        .crypto()
        .sha256(&soroban_sdk::Bytes::from_array(&e, b"the-signed-message"));
    let other = e
        .crypto()
        .sha256(&soroban_sdk::Bytes::from_array(&e, b"a-different-message"));
    let sig = acc.sign(&e, signed.clone());
    let signature = Signature::Ed25519(sig);
    let signer = SignerKey::Ed25519(acc.public_key.clone());

    verify_signature(&e, &signer, &signature, &other).unwrap();
}
