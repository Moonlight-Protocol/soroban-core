#![cfg(test)]
use soroban_sdk::{
    testutils::Address as TestAddress,
    token::{StellarAssetClient, TokenClient},
    vec, Address, BytesN, Env,
};

use utxo::{
    core::{bundle_payload, Bundle},
    tests::helpers::{generate_utxo_keypair, sign_hash},
};

use crate::{contract::PrivacyPoolContractClient, tests::helpers::create_contracts};

#[test]
fn test_delegated_bal_success() {
    let e = Env::default();
    e.mock_all_auths();

    let (_admin, pool, asset_client, token_client): (
        Address,
        PrivacyPoolContractClient,
        StellarAssetClient,
        TokenClient,
    ) = create_contracts(&e);

    let provider_a = <soroban_sdk::Address as TestAddress>::generate(&e);
    let provider_b = <soroban_sdk::Address as TestAddress>::generate(&e);

    pool.register_provider(&provider_a);
    pool.register_provider(&provider_b);

    let amount: i128 = 1000;
    let user = <soroban_sdk::Address as TestAddress>::generate(&e);

    asset_client.mint(&user, &amount);

    let utxo_keypair_a = generate_utxo_keypair(&e);

    pool.deposit(&user, &amount, &utxo_keypair_a.public_key);

    // ============================================
    // Keypair A(1000) sends 250 to Keypair B
    // Provider A gets 750
    // ============================================

    let utxo_keypair_b = generate_utxo_keypair(&e);

    let mut bundle_a = Bundle {
        spend: vec![&e, utxo_keypair_a.public_key.clone()],
        create: vec![&e, (utxo_keypair_b.public_key.clone(), 250)],
        signatures: vec![&e],
    };

    let hash_a = bundle_payload(&e, bundle_a.clone(), "DELEGATED_TRANSFER");

    let signature_a: [u8; 64] = sign_hash(&utxo_keypair_a.secret_key, &hash_a);

    let signature_bytes_a = BytesN::<64>::from_array(&e, &signature_a);

    bundle_a.signatures.insert(0, signature_bytes_a.clone());

    pool.delegated_transfer_bal(&vec![&e, bundle_a.clone()], &provider_a);

    let provider_a_balance = pool.provider_balance(&provider_a);
    assert_eq!(
        provider_a_balance, 750,
        "Expected balance to be equal to change amount"
    );

    let utxo_a_balance_after_transfer = pool.balance(&utxo_keypair_a.public_key.clone());
    assert_eq!(
        utxo_a_balance_after_transfer, 0,
        "Expected balance to be equal to zero after spending"
    );

    let utxo_b_balance_after_transfer = pool.balance(&utxo_keypair_b.public_key.clone());
    assert_eq!(
        utxo_b_balance_after_transfer, 250,
        "Expected balance to be equal to spent amount"
    );

    // ============================================
    // Keypair B(250) sends 100 to Keypair C
    // Provider B gets 150
    // ============================================

    let utxo_keypair_c = generate_utxo_keypair(&e);

    let mut bundle_b = Bundle {
        spend: vec![&e, utxo_keypair_b.public_key.clone()],
        create: vec![&e, (utxo_keypair_c.public_key.clone(), 100)],
        signatures: vec![&e],
    };

    let hash_b = bundle_payload(&e, bundle_b.clone(), "DELEGATED_TRANSFER");

    let signature_b: [u8; 64] = sign_hash(&utxo_keypair_b.secret_key, &hash_b);

    let signature_bytes_b = BytesN::<64>::from_array(&e, &signature_b);

    bundle_b.signatures.insert(0, signature_bytes_b.clone());

    pool.delegated_transfer_bal(&vec![&e, bundle_b.clone()], &provider_b);

    let provider_b_balance = pool.provider_balance(&provider_b);
    assert_eq!(
        provider_b_balance, 150,
        "Expected balance to be equal to change amount"
    );

    let utxo_b_balance_after_second_transfer = pool.balance(&utxo_keypair_b.public_key.clone());

    assert_eq!(
        utxo_b_balance_after_second_transfer, 0,
        "Expected balance to be equal to zero after spending"
    );

    let utxo_c_balance_after_second_transfer = pool.balance(&utxo_keypair_c.public_key.clone());

    assert_eq!(
        utxo_c_balance_after_second_transfer, 100,
        "Expected balance to be equal to spent amount"
    );

    assert_eq!(
        token_client.balance(&provider_a),
        0,
        "Expected provider A balance to be 0 before withdrawing"
    );

    assert_eq!(
        token_client.balance(&provider_b),
        0,
        "Expected provider B balance to be 0 before withdrawing"
    );

    pool.provider_withdraw(&provider_a, &750);

    assert_eq!(
        token_client.balance(&provider_a),
        750,
        "Expected provider A balance to be 750 after withdrawing"
    );

    pool.provider_withdraw(&provider_b, &150);

    assert_eq!(
        token_client.balance(&provider_b),
        150,
        "Expected provider B balance to be 150 after withdrawing"
    );
}

#[test]
fn test_delegated_transfer_bal_without_create_success() {
    let e = Env::default();
    e.mock_all_auths();

    let (_admin, pool, asset_client, _token_client): (
        Address,
        PrivacyPoolContractClient,
        StellarAssetClient,
        TokenClient,
    ) = create_contracts(&e);

    let provider_a = <soroban_sdk::Address as TestAddress>::generate(&e);

    pool.register_provider(&provider_a);

    let amount: i128 = 1000;
    let user = <soroban_sdk::Address as TestAddress>::generate(&e);

    asset_client.mint(&user, &amount);

    let utxo_keypair_a = generate_utxo_keypair(&e);

    pool.deposit(&user, &amount, &utxo_keypair_a.public_key);

    // ============================================
    // Keypair A(1000) spends 1000 without creating any UTXO
    // Provider A gets 1000
    // ============================================

    let mut bundle_a = Bundle {
        spend: vec![&e, utxo_keypair_a.public_key.clone()],
        create: vec![&e],
        signatures: vec![&e],
    };

    let hash_a = bundle_payload(&e, bundle_a.clone(), "DELEGATED_TRANSFER");

    let signature_a: [u8; 64] = sign_hash(&utxo_keypair_a.secret_key, &hash_a);

    let signature_bytes_a = BytesN::<64>::from_array(&e, &signature_a);

    bundle_a.signatures.insert(0, signature_bytes_a.clone());

    pool.delegated_transfer_bal(&vec![&e, bundle_a.clone()], &provider_a);

    let provider_a_balance = pool.provider_balance(&provider_a);
    assert_eq!(
        provider_a_balance, 1000,
        "Expected balance to be equal to change amount"
    );

    let utxo_a_balance_after_transfer = pool.balance(&utxo_keypair_a.public_key.clone());
    assert_eq!(
        utxo_a_balance_after_transfer, 0,
        "Expected balance to be equal to zero after spending"
    );
}

#[test]
#[should_panic]
fn test_delegated_bal_withdraw_missing_auth_failure() {
    let e = Env::default();
    e.mock_all_auths();

    let (_admin, pool, asset_client, _token_client): (
        Address,
        PrivacyPoolContractClient,
        StellarAssetClient,
        TokenClient,
    ) = create_contracts(&e);

    let provider = <soroban_sdk::Address as TestAddress>::generate(&e);

    pool.register_provider(&provider);

    let amount: i128 = 1000;
    let user = <soroban_sdk::Address as TestAddress>::generate(&e);

    asset_client.mint(&user, &amount);

    let utxo_keypair_a = generate_utxo_keypair(&e);

    pool.deposit(&user, &amount, &utxo_keypair_a.public_key);

    // ============================================
    // Keypair A(1000) sends 250 to Keypair B
    // Provider A gets 750
    // ============================================

    let utxo_keypair_b = generate_utxo_keypair(&e);

    let mut bundle_a = Bundle {
        spend: vec![&e, utxo_keypair_a.public_key.clone()],
        create: vec![&e, (utxo_keypair_b.public_key.clone(), 250)],
        signatures: vec![&e],
    };

    let hash_a = bundle_payload(&e, bundle_a.clone(), "DELEGATED_TRANSFER");

    let signature_a: [u8; 64] = sign_hash(&utxo_keypair_a.secret_key, &hash_a);

    let signature_bytes_a = BytesN::<64>::from_array(&e, &signature_a);

    bundle_a.signatures.insert(0, signature_bytes_a.clone());

    pool.delegated_transfer_bal(&vec![&e, bundle_a.clone()], &provider);

    let provider_a_balance = pool.provider_balance(&provider);
    assert_eq!(
        provider_a_balance, 750,
        "Expected balance to be equal to change amount"
    );

    e.set_auths(&[]); // Clear all auths
    pool.provider_withdraw(&provider, &750);
}

#[test]
#[should_panic]
fn test_delegated_bal_not_provider_failure() {
    let e = Env::default();
    e.mock_all_auths();

    let (_admin, pool, asset_client, _token_client): (
        Address,
        PrivacyPoolContractClient,
        StellarAssetClient,
        TokenClient,
    ) = create_contracts(&e);

    let fake_provider = <soroban_sdk::Address as TestAddress>::generate(&e);

    let amount: i128 = 1000;
    let user = <soroban_sdk::Address as TestAddress>::generate(&e);

    asset_client.mint(&user, &amount);

    let utxo_keypair_a = generate_utxo_keypair(&e);

    pool.deposit(&user, &amount, &utxo_keypair_a.public_key);

    // ============================================
    // Keypair A(1000) attempts to send 250 to Keypair B
    // Transaction should fail as fake_provider is not a registered provider
    // ============================================

    let utxo_keypair_b = generate_utxo_keypair(&e);

    let mut bundle_a = Bundle {
        spend: vec![&e, utxo_keypair_a.public_key.clone()],
        create: vec![&e, (utxo_keypair_b.public_key.clone(), 250)],
        signatures: vec![&e],
    };

    let hash_a = bundle_payload(&e, bundle_a.clone(), "DELEGATED_TRANSFER");

    let signature_a: [u8; 64] = sign_hash(&utxo_keypair_a.secret_key, &hash_a);

    let signature_bytes_a = BytesN::<64>::from_array(&e, &signature_a);

    bundle_a.signatures.insert(0, signature_bytes_a.clone());

    pool.delegated_transfer_bal(&vec![&e, bundle_a.clone()], &fake_provider);
}

#[test]
#[should_panic]
fn test_delegated_bal_provider_auth_missing_failure() {
    let e = Env::default();
    e.mock_all_auths();

    let (_admin, pool, asset_client, _token_client): (
        Address,
        PrivacyPoolContractClient,
        StellarAssetClient,
        TokenClient,
    ) = create_contracts(&e);

    let provider = <soroban_sdk::Address as TestAddress>::generate(&e);

    let amount: i128 = 1000;
    let user = <soroban_sdk::Address as TestAddress>::generate(&e);

    asset_client.mint(&user, &amount);

    let utxo_keypair_a = generate_utxo_keypair(&e);

    pool.deposit(&user, &amount, &utxo_keypair_a.public_key);

    // ============================================
    // Keypair A(1000) attempts to send 250 to Keypair B
    // Transaction should fail as provider didn't sign the transaction
    // ============================================

    let utxo_keypair_b = generate_utxo_keypair(&e);

    let mut bundle_a = Bundle {
        spend: vec![&e, utxo_keypair_a.public_key.clone()],
        create: vec![&e, (utxo_keypair_b.public_key.clone(), 250)],
        signatures: vec![&e],
    };

    let hash_a = bundle_payload(&e, bundle_a.clone(), "DELEGATED_TRANSFER");

    let signature_a: [u8; 64] = sign_hash(&utxo_keypair_a.secret_key, &hash_a);

    let signature_bytes_a = BytesN::<64>::from_array(&e, &signature_a);

    bundle_a.signatures.insert(0, signature_bytes_a.clone());

    e.set_auths(&[]); // Clear all auths

    pool.delegated_transfer_bal(&vec![&e, bundle_a.clone()], &provider);
}
