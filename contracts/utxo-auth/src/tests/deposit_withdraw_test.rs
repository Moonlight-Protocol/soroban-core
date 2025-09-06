#![cfg(test)]
use soroban_sdk::{
    testutils::Address as TestAddress,
    token::{StellarAssetClient, TokenClient},
    Address, BytesN, Env,
};

use utxo::tests::helpers::{generate_utxo_keypair, sign_hash};

use crate::{
    contract::{withdraw_payload, PrivacyPoolContractClient},
    tests::helpers::create_contracts,
};

#[test]
fn test_deposit_and_withdraw_success() {
    let e = Env::default();
    e.mock_all_auths();

    let (_admin, pool, asset_client, token_client): (
        Address,
        PrivacyPoolContractClient,
        StellarAssetClient,
        TokenClient,
    ) = create_contracts(&e);

    let amount: i128 = 100;

    let user = <soroban_sdk::Address as TestAddress>::generate(&e);

    asset_client.mint(&user, &amount);
    assert_eq!(
        token_client.balance(&user),
        amount,
        "Expected user balance to be equal to minted amount before initiating test"
    );

    let utxo_keypair = generate_utxo_keypair(&e);

    pool.deposit(&user, &amount, &utxo_keypair.public_key);

    assert_eq!(
        pool.balance(&utxo_keypair.public_key),
        amount,
        "Expected UTXO balance to be equal to deposited amount"
    );
    assert_eq!(
        token_client.balance(&user),
        0,
        "Expected user balance to be 0 after deposit"
    );
    assert_eq!(
        token_client.balance(&pool.address),
        amount,
        "Expected pool balance to be equal to deposited amount"
    );

    let payload = withdraw_payload(&e, utxo_keypair.public_key.clone(), amount.clone());
    let signature = sign_hash(&utxo_keypair.secret_key, &payload);
    let signature_bytes = BytesN::<64>::from_array(&e, &signature);

    pool.withdraw(&user, &amount, &utxo_keypair.public_key, &signature_bytes);

    assert_eq!(
        pool.balance(&utxo_keypair.public_key),
        0,
        "Expected UTXO balance to be 0 after withdraw"
    );
    assert_eq!(
        token_client.balance(&user),
        amount,
        "Expected user balance to be equal to deposited amount after withdraw"
    );
    assert_eq!(
        token_client.balance(&pool.address),
        0,
        "Expected pool balance to be 0 after withdraw"
    );
}
