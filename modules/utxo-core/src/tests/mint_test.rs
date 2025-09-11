#![cfg(test)]

use soroban_sdk::{BytesN, Env, testutils::Events, symbol_short, IntoVal};
use crate::tests::helpers::create_contract;

#[test]
fn test_mint_success() {
    let e = Env::default();

    // Create a TestContract client
    let contract = create_contract(&e);

    // Create a UTXO identifier (mocked with all zeros)
    let utxo = BytesN::<65>::from_array(&e, &[0u8; 65]);

    // Define the amount to mint
    let amount: i128 = 100;

    // Call the mint function via the contract
    contract.mint(&amount, &utxo.clone());


    // Verify that the correct event was emitted
    let events = e.events().all();

    let expected_event = (
        contract.address.clone(),
        (
            utxo.clone(),
            symbol_short!("create")
        ).into_val(&e),
        amount.into_val(&e),
    );
    assert!(events.contains(&expected_event), "Expected mint event to be emitted");


    // Verify that the correct amount was minted
    let balance = contract.utxo_balance(&utxo.clone());
    assert_eq!(balance, amount, "Expected balance to be equal to minted amount");
    
}
