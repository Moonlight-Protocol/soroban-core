#![cfg(test)]

use crate::{
    tests::helpers::{create_contract, generate_utxo_keypair, sign_hash},
    utxo,
};
use soroban_sdk::{vec, BytesN, Env};
#[test]
fn test_transfer_custom_bundle_with_create_success() {
    let e = Env::default();

    let keypair1 = generate_utxo_keypair(&e);
    let keypair2 = generate_utxo_keypair(&e);
    let keypair3 = generate_utxo_keypair(&e);

    let contract = create_contract(&e);

    // Create UTXO identifiers
    let utxo1 = keypair1.public_key.clone();
    let utxo2 = keypair2.public_key.clone();
    let utxo3 = keypair3.public_key.clone();

    // Mint UTXOs: utxo1 gets 100 and utxo2 gets 50
    contract.mint(&100, &utxo1);
    contract.mint(&50, &utxo2);

    // Build a bundle that spends both utxos and creates one UTXO with 120,
    // leaving a leftover of 30 (since 100 + 50 = 150, and 150 - 120 = 30).
    let mut bundle = utxo::Bundle {
        spend: vec![&e, utxo1.clone(), utxo2.clone()],
        create: vec![&e, (utxo3.clone(), 120)],
        signatures: vec![&e],
    };

    // Sign the bundle for action "CUSTOM"
    let hash = utxo::bundle_payload(&e, bundle.clone(), "CUSTOM");
    let signature1 = sign_hash(&keypair1.secret_key, &hash);
    let signature2 = sign_hash(&keypair2.secret_key, &hash);
    let sig1_bytes = BytesN::<64>::from_array(&e, &signature1);
    let sig2_bytes = BytesN::<64>::from_array(&e, &signature2);
    bundle.signatures.insert(0, sig1_bytes);
    bundle.signatures.insert(1, sig2_bytes);

    // Call the new transfer_burn_leftover function with action "CUSTOM"
    let leftover: i128 = contract.transfer_custom_leftover(&vec![&e, bundle.clone()]);

    // The expected leftover is (100 + 50) - 120 = 30.
    assert_eq!(leftover, 30, "Expected leftover amount to be 30");

    // Verify that the original UTXOs have been spent.
    let balance1 = contract.utxo_balance(&utxo1);
    let balance2 = contract.utxo_balance(&utxo2);
    let balance3 = contract.utxo_balance(&utxo3);
    assert_eq!(balance1, 0, "Expected utxo1 to be spent");
    assert_eq!(balance2, 0, "Expected utxo2 to be spent");
    assert_eq!(balance3, 120, "Expected utxo3 to be created with 120");
}

#[test]
fn test_transfer_custom_bundle_without_create_success() {
    let e = Env::default();

    let keypair1 = generate_utxo_keypair(&e);
    let keypair2 = generate_utxo_keypair(&e);

    let contract = create_contract(&e);

    // Create UTXO identifiers
    let utxo1 = keypair1.public_key.clone();
    let utxo2 = keypair2.public_key.clone();

    // Mint UTXOs: utxo1 gets 100 and utxo2 gets 50
    contract.mint(&100, &utxo1);
    contract.mint(&50, &utxo2);

    // Build a bundle that spends both utxos and creates one UTXO with 120,
    // leaving a leftover of 30 (since 100 + 50 = 150, and 150 - 120 = 30).
    let mut bundle = utxo::Bundle {
        spend: vec![&e, utxo1.clone(), utxo2.clone()],
        create: vec![&e],
        signatures: vec![&e],
    };

    // Sign the bundle for action "CUSTOM"
    let hash = utxo::bundle_payload(&e, bundle.clone(), "CUSTOM");
    let signature1 = sign_hash(&keypair1.secret_key, &hash);
    let signature2 = sign_hash(&keypair2.secret_key, &hash);
    let sig1_bytes = BytesN::<64>::from_array(&e, &signature1);
    let sig2_bytes = BytesN::<64>::from_array(&e, &signature2);
    bundle.signatures.insert(0, sig1_bytes);
    bundle.signatures.insert(1, sig2_bytes);

    // Call the new transfer_burn_leftover function with action "CUSTOM"
    let leftover: i128 = contract.transfer_custom_leftover(&vec![&e, bundle.clone()]);

    // The expected leftover is (100 + 50) - 120 = 30.
    assert_eq!(leftover, 150, "Expected leftover amount to be 150");

    // Verify that the original UTXOs have been spent.
    let balance1 = contract.utxo_balance(&utxo1);
    let balance2 = contract.utxo_balance(&utxo2);
    assert_eq!(balance1, 0, "Expected utxo1 to be spent");
    assert_eq!(balance2, 0, "Expected utxo2 to be spent");
}
