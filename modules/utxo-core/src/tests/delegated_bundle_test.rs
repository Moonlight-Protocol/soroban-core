#![cfg(test)]

use crate::{
    core::{bundle_payload, Bundle},
    tests::helpers::{create_contract, generate_utxo_keypair, sign_hash},
};
use soroban_sdk::{symbol_short, testutils::Events, vec, BytesN, Env, IntoVal};

#[test]
fn test_single_delegated_transfer_success() {
    let e = Env::default();

    let keypair1 = generate_utxo_keypair(&e);
    let keypair2 = generate_utxo_keypair(&e);
    let delegate_keypair = generate_utxo_keypair(&e);

    // Create a TestContract client
    let contract = create_contract(&e);

    // Create a UTXO identifier
    let utxo1 = keypair1.public_key.clone();
    let utxo2 = keypair2.public_key.clone();

    // Define the amount to mint
    let amount: i128 = 100;

    // Call the mint function via the contract
    contract.mint(&amount, &utxo1.clone());

    let mut bundle = Bundle {
        spend: vec![&e, utxo1.clone()],
        create: vec![&e, (utxo2.clone(), 80)],
        signatures: vec![&e],
    };

    let hash = bundle_payload(&e, bundle.clone(), "DELEGATED_TRANSFER");

    let signature = sign_hash(&keypair1.secret_key, &hash);

    let signature_bytes = BytesN::<64>::from_array(&e, &signature);

    bundle.signatures.insert(0, signature_bytes.clone());

    contract.delegated_transfer(
        &vec![&e, bundle.clone()],
        &delegate_keypair.public_key.clone(),
    );

    // Verify that the correct event was emitted
    let events = e.events().all();

    let expected_spend_event = (
        contract.address.clone(),
        (utxo1.clone(), symbol_short!("spend")).into_val(&e),
        (100 as i128).into_val(&e),
    );

    assert!(
        events.contains(&expected_spend_event),
        "Expected spend event to be emitted"
    );

    let expected_create_event = (
        contract.address.clone(),
        (utxo2.clone(), symbol_short!("create")).into_val(&e),
        (80 as i128).into_val(&e),
    );

    assert!(
        events.contains(&expected_create_event),
        "Expected create event to be emitted"
    );

    let expected_create_delegate_event = (
        contract.address.clone(),
        (delegate_keypair.public_key.clone(), symbol_short!("create")).into_val(&e),
        (20 as i128).into_val(&e),
    );

    assert!(
        events.contains(&expected_create_delegate_event),
        "Expected create event to be emitted"
    );

    // Verify that the correct amount was burned
    let balance_after_burn = contract.utxo_balance(&utxo1.clone());
    assert_eq!(
        balance_after_burn, 0,
        "Expected balance to be zero after burn"
    );

    // Verify that the correct amount was minted
    let balance_after_mint = contract.utxo_balance(&utxo2.clone());
    assert_eq!(
        balance_after_mint, 80,
        "Expected balance to be equal to minted amount"
    );

    // Verify that the correct amount was minted to the delegate
    let balance_after_mint_delegate = contract.utxo_balance(&delegate_keypair.public_key.clone());
    assert_eq!(
        balance_after_mint_delegate, 20,
        "Expected balance to be equal to minted amount"
    );
}

#[test]
fn test_single_delegated_transfer_only_spending_success() {
    let e = Env::default();

    let keypair1 = generate_utxo_keypair(&e);
    let keypair2 = generate_utxo_keypair(&e);
    let delegate_keypair = generate_utxo_keypair(&e);

    // Create a TestContract client
    let contract = create_contract(&e);

    // Create a UTXO identifier
    let utxo1 = keypair1.public_key.clone();
    let utxo2 = keypair2.public_key.clone();

    // Call the mint function via the contract
    contract.mint(&100_i128, &utxo1.clone());
    contract.mint(&50_i128, &utxo2.clone());

    let mut bundle = Bundle {
        spend: vec![&e, utxo1.clone(), utxo2.clone()],
        create: vec![&e],
        signatures: vec![&e],
    };

    let hash = bundle_payload(&e, bundle.clone(), "DELEGATED_TRANSFER");

    let signature_1 = sign_hash(&keypair1.secret_key, &hash);
    let signature_2 = sign_hash(&keypair2.secret_key, &hash);

    let signature_1_bytes = BytesN::<64>::from_array(&e, &signature_1);
    let signature_2_bytes = BytesN::<64>::from_array(&e, &signature_2);

    bundle.signatures.insert(0, signature_1_bytes.clone());
    bundle.signatures.insert(1, signature_2_bytes.clone());

    contract.delegated_transfer(
        &vec![&e, bundle.clone()],
        &delegate_keypair.public_key.clone(),
    );

    // Verify that the correct event was emitted
    let events = e.events().all();

    let expected_spend_event_1 = (
        contract.address.clone(),
        (utxo1.clone(), symbol_short!("spend")).into_val(&e),
        (100 as i128).into_val(&e),
    );

    assert!(
        events.contains(&expected_spend_event_1),
        "Expected spend event to be emitted for UTXO 1"
    );

    let expected_spend_event_2 = (
        contract.address.clone(),
        (utxo2.clone(), symbol_short!("spend")).into_val(&e),
        (50 as i128).into_val(&e),
    );

    assert!(
        events.contains(&expected_spend_event_2),
        "Expected spend event to be emitted for UTXO 2"
    );

    let expected_create_delegate_event = (
        contract.address.clone(),
        (delegate_keypair.public_key.clone(), symbol_short!("create")).into_val(&e),
        (150 as i128).into_val(&e),
    );

    assert!(
        events.contains(&expected_create_delegate_event),
        "Expected create event to be emitted"
    );

    // Verify that the correct amount was burned
    let balance_after_burn = contract.utxo_balance(&utxo1.clone());
    assert_eq!(
        balance_after_burn, 0,
        "Expected balance to be zero after burn"
    );

    // Verify that the correct amount was minted
    let balance_after_mint = contract.utxo_balance(&utxo2.clone());
    assert_eq!(
        balance_after_mint, 0,
        "Expected balance to be zero after burn"
    );

    // Verify that the correct amount was minted to the delegate
    let balance_after_mint_delegate = contract.utxo_balance(&delegate_keypair.public_key.clone());
    assert_eq!(
        balance_after_mint_delegate, 150,
        "Expected balance to be equal to minted amount"
    );
}

#[test]
fn test_split_delegated_transfer_success() {
    let e = Env::default();

    let keypair1 = generate_utxo_keypair(&e);
    let keypair2 = generate_utxo_keypair(&e);
    let keypair3 = generate_utxo_keypair(&e);

    let delegate_keypair = generate_utxo_keypair(&e);

    // Create a TestContract client
    let contract = create_contract(&e);

    // Create a UTXO identifier
    let utxo1 = keypair1.public_key.clone();
    let utxo2 = keypair2.public_key.clone();
    let utxo3 = keypair3.public_key.clone();

    // Define the amount to mint
    let amount_create: i128 = 100;

    // Call the mint function via the contract
    contract.mint(&amount_create, &utxo1.clone());

    let mut bundle = Bundle {
        spend: vec![&e, utxo1.clone()],
        create: vec![&e, (utxo2.clone(), 60i128), (utxo3.clone(), 30i128)],
        signatures: vec![&e],
    };

    let hash = bundle_payload(&e, bundle.clone(), "DELEGATED_TRANSFER");

    let signature = sign_hash(&keypair1.secret_key, &hash);

    let signature_bytes = BytesN::<64>::from_array(&e, &signature);

    bundle.signatures.insert(0, signature_bytes.clone());

    contract.delegated_transfer(
        &vec![&e, bundle.clone()],
        &delegate_keypair.public_key.clone(),
    );

    // Verify that the correct event was emitted
    let events = e.events().all();

    let expected_spend_event = (
        contract.address.clone(),
        (utxo1.clone(), symbol_short!("spend")).into_val(&e),
        amount_create.into_val(&e),
    );

    assert!(
        events.contains(&expected_spend_event),
        "Expected spend event to be emitted for UTXO 1"
    );

    let expected_create_event_1 = (
        contract.address.clone(),
        (utxo2.clone(), symbol_short!("create")).into_val(&e),
        (60 as i128).clone().into_val(&e),
    );

    assert!(
        events.contains(&expected_create_event_1),
        "Expected create event to be emitted for UTXO 2"
    );

    let expected_create_event_2 = (
        contract.address.clone(),
        (utxo3.clone(), symbol_short!("create")).into_val(&e),
        (30 as i128).clone().into_val(&e),
    );

    assert!(
        events.contains(&expected_create_event_2),
        "Expected create event to be emitted for UTXO 3"
    );

    let expected_create_delegate_event = (
        contract.address.clone(),
        (delegate_keypair.public_key.clone(), symbol_short!("create")).into_val(&e),
        (10 as i128).clone().into_val(&e),
    );

    assert!(
        events.contains(&expected_create_delegate_event),
        "Expected create event to be emitted for delegate"
    );

    // Verify that the correct amount
    let balance_after_burn = contract.utxo_balance(&utxo1.clone());
    assert_eq!(
        balance_after_burn, 0,
        "Expected balance to be zero after burn"
    );

    let balance_after_mint1 = contract.utxo_balance(&utxo2.clone());
    assert_eq!(
        balance_after_mint1, 60,
        "Expected balance to be equal to minted amount"
    );

    let balance_after_mint2 = contract.utxo_balance(&utxo3.clone());
    assert_eq!(
        balance_after_mint2, 30,
        "Expected balance to be equal to minted amount"
    );

    let balance_after_mint_delegate = contract.utxo_balance(&delegate_keypair.public_key.clone());
    assert_eq!(
        balance_after_mint_delegate, 10,
        "Expected balance to be equal to minted amount"
    );
}

#[test]
fn test_converging_delegated_transfer_success() {
    let e = Env::default();

    let keypair1 = generate_utxo_keypair(&e);
    let keypair2 = generate_utxo_keypair(&e);
    let keypair3 = generate_utxo_keypair(&e);

    let delegate_keypair = generate_utxo_keypair(&e);

    // Create a TestContract client
    let contract = create_contract(&e);

    // Create a UTXO identifier
    let utxo1 = keypair1.public_key.clone();
    let utxo2 = keypair2.public_key.clone();
    let utxo3 = keypair3.public_key.clone();

    // Define the amount to mint
    let amount_create: i128 = 100;

    // Call the mint function via the contract
    contract.mint(&amount_create, &utxo1.clone());
    contract.mint(&amount_create, &utxo2.clone());

    let mut bundle = Bundle {
        spend: vec![&e, utxo1.clone(), utxo2.clone()],
        create: vec![&e, (utxo3.clone(), 185)],
        signatures: vec![&e],
    };

    let hash = bundle_payload(&e, bundle.clone(), "DELEGATED_TRANSFER");

    let signature1 = sign_hash(&keypair1.secret_key, &hash);
    let signature2 = sign_hash(&keypair2.secret_key, &hash);

    let signature_bytes1 = BytesN::<64>::from_array(&e, &signature1);
    let signature_bytes2 = BytesN::<64>::from_array(&e, &signature2);

    bundle.signatures.insert(0, signature_bytes1.clone());
    bundle.signatures.insert(1, signature_bytes2.clone());

    contract.delegated_transfer(
        &vec![&e, bundle.clone()],
        &delegate_keypair.public_key.clone(),
    );

    // Verify that the correct events were emitted
    let events = e.events().all();

    let expected_spend_event1 = (
        contract.address.clone(),
        (utxo1.clone(), symbol_short!("spend")).into_val(&e),
        amount_create.into_val(&e),
    );

    let expected_spend_event2 = (
        contract.address.clone(),
        (utxo2.clone(), symbol_short!("spend")).into_val(&e),
        amount_create.into_val(&e),
    );

    let expected_create_event = (
        contract.address.clone(),
        (utxo3.clone(), symbol_short!("create")).into_val(&e),
        (185 as i128).clone().into_val(&e),
    );

    let expected_create_delegate_event = (
        contract.address.clone(),
        (delegate_keypair.public_key.clone(), symbol_short!("create")).into_val(&e),
        (15 as i128).clone().into_val(&e),
    );

    assert!(
        events.contains(&expected_spend_event1),
        "Expected spend event to be emitted for UTXO 1"
    );
    assert!(
        events.contains(&expected_spend_event2),
        "Expected spend event to be emitted for UTXO 2"
    );
    assert!(
        events.contains(&expected_create_event),
        "Expected create event to be emitted for UTXO 3"
    );
    assert!(
        events.contains(&expected_create_delegate_event),
        "Expected create event to be emitted for delegate"
    );

    // Verify that the correct amount
    let balance_after_burn1 = contract.utxo_balance(&utxo1.clone());
    assert_eq!(
        balance_after_burn1, 0,
        "Expected balance to be zero after burn"
    );

    let balance_after_burn2 = contract.utxo_balance(&utxo2.clone());
    assert_eq!(
        balance_after_burn2, 0,
        "Expected balance to be zero after burn"
    );

    let balance_after_mint = contract.utxo_balance(&utxo3.clone());
    assert_eq!(
        balance_after_mint, 185,
        "Expected balance to be equal to minted amount"
    );

    let balance_after_mint_delegate = contract.utxo_balance(&delegate_keypair.public_key.clone());
    assert_eq!(
        balance_after_mint_delegate, 15,
        "Expected balance to be equal to minted amount"
    );
}

#[test]
fn test_mixed_balance_delegated_transfer_successfull() {
    let e = Env::default();

    let keypair1 = generate_utxo_keypair(&e);
    let keypair2 = generate_utxo_keypair(&e);
    let keypair3 = generate_utxo_keypair(&e);
    let keypair4 = generate_utxo_keypair(&e);
    let keypair5 = generate_utxo_keypair(&e);

    let delegate_keypair = generate_utxo_keypair(&e);

    // Create a TestContract client
    let contract = create_contract(&e);

    // Create a UTXO identifier
    let utxo1 = keypair1.public_key.clone();
    let utxo2 = keypair2.public_key.clone();
    let utxo3 = keypair3.public_key.clone();
    let utxo4 = keypair4.public_key.clone();
    let utxo5 = keypair5.public_key.clone();

    // Call the mint function via the contract
    contract.mint(&200, &utxo1.clone());
    contract.mint(&300, &utxo2.clone());

    let mut bundle = Bundle {
        spend: vec![&e, utxo1.clone(), utxo2.clone()],
        create: vec![
            &e,
            (utxo3.clone(), 100),
            (utxo4.clone(), 245),
            (utxo5.clone(), 145),
        ],
        signatures: vec![&e],
    };

    let hash = bundle_payload(&e, bundle.clone(), "DELEGATED_TRANSFER");

    let signature1 = sign_hash(&keypair1.secret_key, &hash);
    let signature2 = sign_hash(&keypair2.secret_key, &hash);

    let signature_bytes1 = BytesN::<64>::from_array(&e, &signature1);
    let signature_bytes2 = BytesN::<64>::from_array(&e, &signature2);

    bundle.signatures.insert(0, signature_bytes1.clone());
    bundle.signatures.insert(1, signature_bytes2.clone());

    contract.delegated_transfer(
        &vec![&e, bundle.clone()],
        &delegate_keypair.public_key.clone(),
    );

    // Verify that the correct events were emitted
    let events = e.events().all();

    let expected_spend_event1 = (
        contract.address.clone(),
        (utxo1.clone(), symbol_short!("spend")).into_val(&e),
        (200 as i128).clone().into_val(&e),
    );

    let expected_spend_event2 = (
        contract.address.clone(),
        (utxo2.clone(), symbol_short!("spend")).into_val(&e),
        (300 as i128).clone().into_val(&e),
    );

    let expected_create_event = (
        contract.address.clone(),
        (utxo3.clone(), symbol_short!("create")).into_val(&e),
        (100 as i128).clone().into_val(&e),
    );

    let expected_create_event2 = (
        contract.address.clone(),
        (utxo4.clone(), symbol_short!("create")).into_val(&e),
        (245 as i128).clone().into_val(&e),
    );

    let expected_create_event3 = (
        contract.address.clone(),
        (utxo5.clone(), symbol_short!("create")).into_val(&e),
        (145 as i128).clone().into_val(&e),
    );

    let expected_create_delegate_event = (
        contract.address.clone(),
        (delegate_keypair.public_key.clone(), symbol_short!("create")).into_val(&e),
        (10 as i128).clone().into_val(&e),
    );

    assert!(
        events.contains(&expected_spend_event1),
        "Expected spend event to be emitted for UTXO 1"
    );
    assert!(
        events.contains(&expected_spend_event2),
        "Expected spend event to be emitted for UTXO 2"
    );
    assert!(
        events.contains(&expected_create_event),
        "Expected create event to be emitted for UTXO 3"
    );
    assert!(
        events.contains(&expected_create_event2),
        "Expected create event to be emitted for UTXO 4"
    );
    assert!(
        events.contains(&expected_create_event3),
        "Expected create event to be emitted for UTXO 5"
    );
    assert!(
        events.contains(&expected_create_delegate_event),
        "Expected create event to be emitted for delegate"
    );

    // Verify that the correct amount
    let balance_after_burn1 = contract.utxo_balance(&utxo1.clone());
    assert_eq!(
        balance_after_burn1, 0,
        "Expected balance to be zero after burn"
    );

    let balance_after_burn2 = contract.utxo_balance(&utxo2.clone());
    assert_eq!(
        balance_after_burn2, 0,
        "Expected balance to be zero after burn"
    );

    let balance_after_mint = contract.utxo_balance(&utxo3.clone());
    assert_eq!(
        balance_after_mint, 100,
        "Expected balance to be equal to minted amount"
    );

    let balance_after_mint2 = contract.utxo_balance(&utxo4.clone());
    assert_eq!(
        balance_after_mint2, 245,
        "Expected balance to be equal to minted amount"
    );

    let balance_after_mint3 = contract.utxo_balance(&utxo5.clone());
    assert_eq!(
        balance_after_mint3, 145,
        "Expected balance to be equal to minted amount"
    );

    let balance_after_mint_delegate = contract.utxo_balance(&delegate_keypair.public_key.clone());
    assert_eq!(
        balance_after_mint_delegate, 10,
        "Expected balance to be equal to minted amount"
    );
}
