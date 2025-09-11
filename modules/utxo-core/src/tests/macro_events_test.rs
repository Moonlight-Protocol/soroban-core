#![cfg(test)]

use crate::tests::helpers::create_contract;
use soroban_sdk::{symbol_short, testutils::Events, BytesN, Env, IntoVal};

#[test]
fn test_mint_event_controlled_by_macro() {
    let e = Env::default();
    let contract = create_contract(&e);

    let utxo = BytesN::<65>::from_array(&e, &[0u8; 65]);
    let amount: i128 = 100;

    contract.mint(&amount, &utxo);

    let events = e.events().all();

    let expected_event = (
        contract.address.clone(),
        (utxo.clone(), symbol_short!("create")).into_val(&e),
        amount.into_val(&e),
    );

    // to test this variation, use
    // use the following command:
    // make test-no-utxo-events
    #[cfg(feature = "no-utxo-events")]
    assert!(
        !events.contains(&expected_event),
        "UTXO event should be suppressed"
    );

    #[cfg(not(feature = "no-utxo-events"))]
    assert!(
        events.contains(&expected_event),
        "Expected mint event to be emitted"
    );
}
