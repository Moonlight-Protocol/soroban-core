#![cfg(test)]

use soroban_sdk::{BytesN, Env, testutils::Events, symbol_short, IntoVal};
use crate::{tests::helpers::{create_contract, generate_utxo_keypair, sign_hash}, utxo};



#[test]
fn test_burn_success() {
    let e = Env::default();

    let keypair = generate_utxo_keypair(&e);

    // Create a TestContract client
    let contract = create_contract(&e);

    // Create a UTXO identifier (mocked with all zeros)
    let utxo = keypair.public_key.clone();//BytesN::<65>::from_array(&e, &[0u8; 65]);

    // Define the amount to mint
    let amount: i128 = 100;

    // Call the mint function via the contract
    contract.mint(&amount, &utxo.clone());

    // Sign the burn payload
    let hash = utxo::burn_payload(&e, &utxo, amount);
    let signature = sign_hash(&keypair.secret_key, &hash);    
    let signature_bytes = BytesN::<64>::from_array(&e, &signature);
    

    // Call the burn function via the contract
    contract.burn( &utxo.clone(), &signature_bytes);

    // Verify that the correct event was emitted
    let events = e.events().all();

    let expected_event = (
        contract.address.clone(),
        (
            utxo.clone(),
            symbol_short!("spend")
        ).into_val(&e),
        amount.into_val(&e),
    );
    assert!(events.contains(&expected_event), "Expected burn event to be emitted");

    // Verify that the correct amount was burned
    let balance_after_burn = contract.utxo_balance(&utxo.clone());
    assert_eq!(balance_after_burn, 0, "Expected balance to be zero after burn");
}