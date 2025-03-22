#![cfg(test)]

use crate::{
    tests::helpers::{create_contract, generate_utxo_keypair, sign_hash},
    utxo,
};
use soroban_sdk::{symbol_short, testutils::Events, vec, BytesN, Env, IntoVal};

#[test]
fn test_multiple_bundle_transactions_successfull() {
    let e = Env::default();

    let keypair_1 = generate_utxo_keypair(&e);

    // Create a TestContract client
    let contract = create_contract(&e);

    // Create a UTXO identifier (mocked with all zeros)
    let utxo_1 = keypair_1.public_key.clone(); //BytesN::<65>::from_array(&e, &[0u8; 65]);

    // Define the amount to mint
    let amount: i128 = 1000;

    // Call the mint function via the contract
    contract.mint(&amount, &utxo_1.clone());

    //
    // Bundle transaction 1
    // Bundle 1:
    // Spend  UTXO 1
    // Create UTXOs 2 and 3
    //

    let keypair_2 = generate_utxo_keypair(&e);
    let keypair_3 = generate_utxo_keypair(&e);

    let utxo_2 = keypair_2.public_key.clone();
    let utxo_3 = keypair_3.public_key.clone();

    let mut bundle = utxo::Bundle {
        spend: vec![&e, utxo_1.clone()],
        create: vec![&e, (utxo_2.clone(), 350), (utxo_3.clone(), 650)],
        signatures: vec![&e],
    };

    let hash_1 = utxo::bundle_payload(&e, bundle.clone(), "TRANSFER");

    let signature_1 = sign_hash(&keypair_1.secret_key, &hash_1);
    let signature_bytes_1 = BytesN::<64>::from_array(&e, &signature_1);

    bundle.signatures = vec![&e, signature_bytes_1.clone()];

    contract.transfer(&vec![&e, bundle.clone()]);

    let events = e.events().all();
    let expected_event_1_1 = (
        contract.address.clone(),
        (utxo_1.clone(), symbol_short!("spend")).into_val(&e),
        (1000 as i128).into_val(&e),
    );

    let expected_event_1_2 = (
        contract.address.clone(),
        (utxo_2.clone(), symbol_short!("create")).into_val(&e),
        (350 as i128).into_val(&e),
    );

    let expected_event_1_3 = (
        contract.address.clone(),
        (utxo_3.clone(), symbol_short!("create")).into_val(&e),
        (650 as i128).into_val(&e),
    );

    assert!(
        events.contains(&expected_event_1_1),
        "Expected spend event to be emitted"
    );
    assert!(
        events.contains(&expected_event_1_2),
        "Expected create event to be emitted"
    );
    assert!(
        events.contains(&expected_event_1_3),
        "Expected create event to be emitted"
    );

    let balance_after_bundle_1_1 = contract.utxo_balance(&utxo_1.clone());
    assert_eq!(
        balance_after_bundle_1_1, 0,
        "Expected balance to be zero after bundle 1"
    );

    let balance_after_bundle_1_2 = contract.utxo_balance(&utxo_2.clone());
    assert_eq!(
        balance_after_bundle_1_2, 350,
        "Expected balance to be 350 after bundle 1"
    );

    let balance_after_bundle_1_3 = contract.utxo_balance(&utxo_3.clone());
    assert_eq!(
        balance_after_bundle_1_3, 650,
        "Expected balance to be 650 after bundle 1"
    );

    //
    // Bundle transaction 2
    // Bundle 2:
    // Spend UTXO 2
    // Create UTXO 4
    // Create UTXO 5
    //
    // Bundle 3:
    // Spend UTXO 3
    // Create UTXO 6
    // Create UTXO 7
    // Create UTXO 8
    //

    let keypair_4 = generate_utxo_keypair(&e);
    let keypair_5 = generate_utxo_keypair(&e);
    let keypair_6 = generate_utxo_keypair(&e);
    let keypair_7 = generate_utxo_keypair(&e);
    let keypair_8 = generate_utxo_keypair(&e);

    let utxo_4 = keypair_4.public_key.clone();
    let utxo_5 = keypair_5.public_key.clone();
    let utxo_6 = keypair_6.public_key.clone();
    let utxo_7 = keypair_7.public_key.clone();
    let utxo_8 = keypair_8.public_key.clone();

    let mut bundle_2 = utxo::Bundle {
        spend: vec![&e, utxo_2.clone()],
        create: vec![&e, (utxo_4.clone(), 200), (utxo_5.clone(), 150)],
        signatures: vec![&e],
    };

    let mut bundle_3 = utxo::Bundle {
        spend: vec![&e, utxo_3.clone()],
        create: vec![
            &e,
            (utxo_6.clone(), 300),
            (utxo_7.clone(), 200),
            (utxo_8.clone(), 150),
        ],
        signatures: vec![&e],
    };

    let hash_2 = utxo::bundle_payload(&e, bundle_2.clone(), "TRANSFER");
    let hash_3 = utxo::bundle_payload(&e, bundle_3.clone(), "TRANSFER");

    let signature_2 = sign_hash(&keypair_2.secret_key, &hash_2);
    let signature_bytes_2 = BytesN::<64>::from_array(&e, &signature_2);

    let signature_3 = sign_hash(&keypair_3.secret_key, &hash_3);
    let signature_bytes_3 = BytesN::<64>::from_array(&e, &signature_3);

    bundle_2.signatures = vec![&e, signature_bytes_2.clone()];
    bundle_3.signatures = vec![&e, signature_bytes_3.clone()];

    contract.transfer(&vec![&e, bundle_2.clone(), bundle_3.clone()]);

    let events = e.events().all();

    let expected_event_2_1 = (
        contract.address.clone(),
        (utxo_2.clone(), symbol_short!("spend")).into_val(&e),
        (350 as i128).into_val(&e),
    );

    let expected_event_2_2 = (
        contract.address.clone(),
        (utxo_4.clone(), symbol_short!("create")).into_val(&e),
        (200 as i128).into_val(&e),
    );

    let expected_event_2_3 = (
        contract.address.clone(),
        (utxo_5.clone(), symbol_short!("create")).into_val(&e),
        (150 as i128).into_val(&e),
    );

    let expected_event_3_1 = (
        contract.address.clone(),
        (utxo_3.clone(), symbol_short!("spend")).into_val(&e),
        (650 as i128).into_val(&e),
    );

    let expected_event_3_2 = (
        contract.address.clone(),
        (utxo_6.clone(), symbol_short!("create")).into_val(&e),
        (300 as i128).into_val(&e),
    );

    let expected_event_3_3 = (
        contract.address.clone(),
        (utxo_7.clone(), symbol_short!("create")).into_val(&e),
        (200 as i128).into_val(&e),
    );

    let expected_event_3_4 = (
        contract.address.clone(),
        (utxo_8.clone(), symbol_short!("create")).into_val(&e),
        (150 as i128).into_val(&e),
    );

    assert!(
        events.contains(&expected_event_2_1),
        "Expected spend event to be emitted"
    );
    assert!(
        events.contains(&expected_event_2_2),
        "Expected create event to be emitted"
    );
    assert!(
        events.contains(&expected_event_2_3),
        "Expected create event to be emitted"
    );

    assert!(
        events.contains(&expected_event_3_1),
        "Expected spend event to be emitted"
    );
    assert!(
        events.contains(&expected_event_3_2),
        "Expected create event to be emitted"
    );
    assert!(
        events.contains(&expected_event_3_3),
        "Expected create event to be emitted"
    );
    assert!(
        events.contains(&expected_event_3_4),
        "Expected create event to be emitted"
    );

    let balance_after_bundle_2_1 = contract.utxo_balance(&utxo_2.clone());
    assert_eq!(
        balance_after_bundle_2_1, 0,
        "Expected balance to be zero after bundle 2"
    );

    let balance_after_bundle_2_2 = contract.utxo_balance(&utxo_4.clone());
    assert_eq!(
        balance_after_bundle_2_2, 200,
        "Expected balance to be 200 after bundle 2"
    );

    let balance_after_bundle_2_3 = contract.utxo_balance(&utxo_5.clone());
    assert_eq!(
        balance_after_bundle_2_3, 150,
        "Expected balance to be 150 after bundle 2"
    );

    let balance_after_bundle_3_1 = contract.utxo_balance(&utxo_3.clone());
    assert_eq!(
        balance_after_bundle_3_1, 0,
        "Expected balance to be zero after bundle 3"
    );

    let balance_after_bundle_3_2 = contract.utxo_balance(&utxo_6.clone());
    assert_eq!(
        balance_after_bundle_3_2, 300,
        "Expected balance to be 300 after bundle 3"
    );

    let balance_after_bundle_3_3 = contract.utxo_balance(&utxo_7.clone());
    assert_eq!(
        balance_after_bundle_3_3, 200,
        "Expected balance to be 200 after bundle 3"
    );

    let balance_after_bundle_3_4 = contract.utxo_balance(&utxo_8.clone());
    assert_eq!(
        balance_after_bundle_3_4, 150,
        "Expected balance to be 150 after bundle 3"
    );

    //
    // Bundle transaction 4
    // Bundle 4:
    // Spend UTXO 4
    // Create UTXO 9
    // Create UTXO 10
    //
    // Bundle 5:
    // Spend UTXO 5
    // Spend UTXO 6
    // Create UTXO 11
    //
    // Bundle 6:
    // Spend UTXO 7
    // Spend UTXO 8
    // Create UTXO 12
    // Create UTXO 13
    //

    let keypair_9 = generate_utxo_keypair(&e);
    let keypair_10 = generate_utxo_keypair(&e);
    let keypair_11 = generate_utxo_keypair(&e);
    let keypair_12 = generate_utxo_keypair(&e);
    let keypair_13 = generate_utxo_keypair(&e);

    let utxo_9 = keypair_9.public_key.clone();
    let utxo_10 = keypair_10.public_key.clone();
    let utxo_11 = keypair_11.public_key.clone();
    let utxo_12 = keypair_12.public_key.clone();
    let utxo_13 = keypair_13.public_key.clone();

    let mut bundle_4 = utxo::Bundle {
        spend: vec![&e, utxo_4.clone()],
        create: vec![&e, (utxo_9.clone(), 100), (utxo_10.clone(), 100)],
        signatures: vec![&e],
    };

    let mut bundle_5 = utxo::Bundle {
        spend: vec![&e, utxo_5.clone(), utxo_6.clone()],
        create: vec![&e, (utxo_11.clone(), 450)],
        signatures: vec![&e],
    };

    let mut bundle_6 = utxo::Bundle {
        spend: vec![&e, utxo_7.clone(), utxo_8.clone()],
        create: vec![&e, (utxo_12.clone(), 200), (utxo_13.clone(), 150)],
        signatures: vec![&e],
    };

    let hash_4 = utxo::bundle_payload(&e, bundle_4.clone(), "TRANSFER");
    let hash_5 = utxo::bundle_payload(&e, bundle_5.clone(), "TRANSFER");
    let hash_6 = utxo::bundle_payload(&e, bundle_6.clone(), "TRANSFER");

    let signature_4 = sign_hash(&keypair_4.secret_key, &hash_4);
    let signature_bytes_4 = BytesN::<64>::from_array(&e, &signature_4);

    let signature_5 = sign_hash(&keypair_5.secret_key, &hash_5);
    let signature_bytes_5 = BytesN::<64>::from_array(&e, &signature_5);

    let signature_6 = sign_hash(&keypair_6.secret_key, &hash_5);
    let signature_bytes_6 = BytesN::<64>::from_array(&e, &signature_6);

    let signature_7 = sign_hash(&keypair_7.secret_key, &hash_6);
    let signature_bytes_7 = BytesN::<64>::from_array(&e, &signature_7);

    let signature_8 = sign_hash(&keypair_8.secret_key, &hash_6);
    let signature_bytes_8 = BytesN::<64>::from_array(&e, &signature_8);

    bundle_4.signatures = vec![&e, signature_bytes_4.clone()];
    bundle_5.signatures = vec![&e, signature_bytes_5.clone(), signature_bytes_6.clone()];
    bundle_6.signatures = vec![&e, signature_bytes_7.clone(), signature_bytes_8.clone()];

    contract.transfer(&vec![
        &e,
        bundle_4.clone(),
        bundle_5.clone(),
        bundle_6.clone(),
    ]);

    let events = e.events().all();

    let expected_event_4_1 = (
        contract.address.clone(),
        (utxo_4.clone(), symbol_short!("spend")).into_val(&e),
        (200 as i128).into_val(&e),
    );

    let expected_event_4_2 = (
        contract.address.clone(),
        (utxo_9.clone(), symbol_short!("create")).into_val(&e),
        (100 as i128).into_val(&e),
    );

    let expected_event_4_3 = (
        contract.address.clone(),
        (utxo_10.clone(), symbol_short!("create")).into_val(&e),
        (100 as i128).into_val(&e),
    );

    let expected_event_5_1 = (
        contract.address.clone(),
        (utxo_5.clone(), symbol_short!("spend")).into_val(&e),
        (150 as i128).into_val(&e),
    );

    let expected_event_5_2 = (
        contract.address.clone(),
        (utxo_6.clone(), symbol_short!("spend")).into_val(&e),
        (300 as i128).into_val(&e),
    );

    let expected_event_5_3 = (
        contract.address.clone(),
        (utxo_11.clone(), symbol_short!("create")).into_val(&e),
        (450 as i128).into_val(&e),
    );

    let expected_event_6_1 = (
        contract.address.clone(),
        (utxo_7.clone(), symbol_short!("spend")).into_val(&e),
        (200 as i128).into_val(&e),
    );

    let expected_event_6_2 = (
        contract.address.clone(),
        (utxo_8.clone(), symbol_short!("spend")).into_val(&e),
        (150 as i128).into_val(&e),
    );

    let expected_event_6_3 = (
        contract.address.clone(),
        (utxo_12.clone(), symbol_short!("create")).into_val(&e),
        (200 as i128).into_val(&e),
    );

    let expected_event_6_4 = (
        contract.address.clone(),
        (utxo_13.clone(), symbol_short!("create")).into_val(&e),
        (150 as i128).into_val(&e),
    );

    assert!(
        events.contains(&expected_event_4_1),
        "Expected spend event to be emitted"
    );
    assert!(
        events.contains(&expected_event_4_2),
        "Expected create event to be emitted"
    );
    assert!(
        events.contains(&expected_event_4_3),
        "Expected create event to be emitted"
    );

    assert!(
        events.contains(&expected_event_5_1),
        "Expected spend event to be emitted"
    );
    assert!(
        events.contains(&expected_event_5_2),
        "Expected spend event to be emitted"
    );
    assert!(
        events.contains(&expected_event_5_3),
        "Expected create event to be emitted"
    );

    assert!(
        events.contains(&expected_event_6_1),
        "Expected spend event to be emitted"
    );
    assert!(
        events.contains(&expected_event_6_2),
        "Expected spend event to be emitted"
    );
    assert!(
        events.contains(&expected_event_6_3),
        "Expected create event to be emitted"
    );
    assert!(
        events.contains(&expected_event_6_4),
        "Expected create event to be emitted"
    );

    let balance_after_bundle_4_1 = contract.utxo_balance(&utxo_4.clone());
    assert_eq!(
        balance_after_bundle_4_1, 0,
        "Expected balance to be zero after bundle 4"
    );

    let balance_after_bundle_4_2 = contract.utxo_balance(&utxo_9.clone());
    assert_eq!(
        balance_after_bundle_4_2, 100,
        "Expected balance to be 100 after bundle 4"
    );

    let balance_after_bundle_4_3 = contract.utxo_balance(&utxo_10.clone());
    assert_eq!(
        balance_after_bundle_4_3, 100,
        "Expected balance to be 100 after bundle 4"
    );

    let balance_after_bundle_5_1 = contract.utxo_balance(&utxo_5.clone());
    assert_eq!(
        balance_after_bundle_5_1, 0,
        "Expected balance to be zero after bundle 5"
    );

    let balance_after_bundle_5_2 = contract.utxo_balance(&utxo_6.clone());
    assert_eq!(
        balance_after_bundle_5_2, 0,
        "Expected balance to be zero after bundle 5"
    );

    let balance_after_bundle_5_3 = contract.utxo_balance(&utxo_11.clone());
    assert_eq!(
        balance_after_bundle_5_3, 450,
        "Expected balance to be 200 after bundle 5"
    );

    let balance_after_bundle_6_1 = contract.utxo_balance(&utxo_7.clone());
    assert_eq!(
        balance_after_bundle_6_1, 0,
        "Expected balance to be zero after bundle 6"
    );

    let balance_after_bundle_6_2 = contract.utxo_balance(&utxo_8.clone());
    assert_eq!(
        balance_after_bundle_6_2, 0,
        "Expected balance to be zero after bundle 6"
    );

    let balance_after_bundle_6_3 = contract.utxo_balance(&utxo_12.clone());
    assert_eq!(
        balance_after_bundle_6_3, 200,
        "Expected balance to be 200 after bundle 6"
    );

    let balance_after_bundle_6_4 = contract.utxo_balance(&utxo_13.clone());
    assert_eq!(
        balance_after_bundle_6_4, 150,
        "Expected balance to be 150 after bundle 6"
    );
}
