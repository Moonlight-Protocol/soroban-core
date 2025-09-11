#![cfg(test)]

use crate::{core::MintRequest, tests::helpers::create_contract};
use soroban_sdk::{symbol_short, testutils::Events, vec, BytesN, Env, IntoVal};

#[test]
fn test_mint_batch_success() {
    let e = Env::default();
    let contract = create_contract(&e);

    let utxo1 = BytesN::<65>::from_array(&e, &[0u8; 65]);
    let utxo2 = BytesN::<65>::from_array(&e, &[1u8; 65]);
    let amount1: i128 = 100;
    let amount2: i128 = 200;

    let requests = vec![
        &e,
        MintRequest {
            utxo: utxo1.clone(),
            amount: amount1,
        },
        MintRequest {
            utxo: utxo2.clone(),
            amount: amount2,
        },
    ];

    contract.mint_batch(&requests);

    let events = e.events().all();
    let expected_event1 = (
        contract.address.clone(),
        (utxo1.clone(), symbol_short!("create")).into_val(&e),
        amount1.into_val(&e),
    );
    let expected_event2 = (
        contract.address.clone(),
        (utxo2.clone(), symbol_short!("create")).into_val(&e),
        amount2.into_val(&e),
    );

    assert!(
        events.contains(&expected_event1),
        "Expected mint event for utxo1 to be emitted"
    );
    assert!(
        events.contains(&expected_event2),
        "Expected mint event for utxo2 to be emitted"
    );

    let balance1 = contract.utxo_balance(&utxo1);
    let balance2 = contract.utxo_balance(&utxo2);
    assert_eq!(balance1, amount1, "Expected balance for utxo1");
    assert_eq!(balance2, amount2, "Expected balance for utxo2");
}
