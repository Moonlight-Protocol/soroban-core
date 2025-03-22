#![cfg(test)]

use crate::{
    tests::helpers::{create_contract, generate_utxo_keypair, sign_hash},
    utxo::{self, BurnRequest},
};
use soroban_sdk::{symbol_short, testutils::Events, vec, BytesN, Env, IntoVal};

#[test]
fn test_burn_batch_success() {
    let e = Env::default();
    let keypair = generate_utxo_keypair(&e);
    let contract = create_contract(&e);

    let utxo = keypair.public_key.clone();
    let amount: i128 = 100;

    contract.mint(&amount, &utxo);

    let hash = utxo::burn_payload(&e, &utxo, amount);
    let signature = sign_hash(&keypair.secret_key, &hash);
    let signature_bytes = BytesN::<64>::from_array(&e, &signature);

    let requests = vec![
        &e,
        BurnRequest {
            utxo: utxo.clone(),
            signature: signature_bytes.clone(),
        },
    ];

    contract.burn_batch(&requests);

    let events = e.events().all();
    let expected_event = (
        contract.address.clone(),
        (utxo.clone(), symbol_short!("spend")).into_val(&e),
        amount.into_val(&e),
    );
    assert!(
        events.contains(&expected_event),
        "Expected burn event to be emitted"
    );

    let balance_after_burn = contract.utxo_balance(&utxo);
    assert_eq!(
        balance_after_burn, 0,
        "Expected balance to be zero after burn"
    );
}
