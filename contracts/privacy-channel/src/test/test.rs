#![cfg(test)]
extern crate std;

use crate::{
    contract::{PrivacyChannelContract, PrivacyChannelContractArgs, PrivacyChannelContractClient},
    test::channel_operation_builder::ChannelOperationBuilder,
};

use channel_auth_contract::contract::{
    ChannelAuthContract, ChannelAuthContractArgs, ChannelAuthContractClient,
};

use moonlight_helpers::testutils::{
    keys::P256KeyPair,
    snapshot::{get_env_with_g_accounts, get_snapshot_g_accounts},
};
use moonlight_primitives::Condition;
use soroban_sdk::{
    testutils::{Address as _, Ledger, MockAuth, MockAuthInvoke},
    vec, Address, Env, FromVal, IntoVal, String,
};

use token_contract::{TestToken as Token, TestTokenClient as TokenClient};

pub fn create_contracts(
    e: &Env,
) -> (
    PrivacyChannelContractClient,
    ChannelAuthContractClient,
    TokenClient,
    Address,
) {
    let admin = Address::generate(&e);

    let auth_contract_id = e.register(
        ChannelAuthContract,
        ChannelAuthContractArgs::__constructor(&admin),
    );
    let auth_contract = ChannelAuthContractClient::new(e, &auth_contract_id);

    let token_address = e.register(
        Token,
        (
            admin.clone(),
            7_u32,
            String::from_val(e, &"Moon Token"),
            String::from_val(e, &"MOON"),
        ),
    );

    let token = TokenClient::new(&e, &token_address);
    // let token_admin = TokenAdminClient::new(&e, &token_address);

    e.mock_all_auths();
    let privacy_channel_contract_id = e.register(
        PrivacyChannelContract,
        PrivacyChannelContractArgs::__constructor(&admin, &auth_contract_id, &token_address),
    );

    e.set_auths(&[]);

    let privacy_channel_contract =
        PrivacyChannelContractClient::new(e, &privacy_channel_contract_id);

    (privacy_channel_contract, auth_contract, token, admin)
}

#[test]
fn test_single_deposit_with_auth() {
    let e = get_env_with_g_accounts();
    let (provider_a, john, _, _, _) = get_snapshot_g_accounts(&e);

    let (channel, auth, token, _) = create_contracts(&e);

    assert_eq!(auth.is_provider(&provider_a.address), false);
    auth.mock_all_auths().add_provider(&provider_a.address);
    assert_eq!(auth.is_provider(&provider_a.address), true);

    token
        .mock_all_auths()
        .mint(&&john.address.clone(), &1000_i128);

    assert_eq!(token.balance(&john.address), 1000_i128);

    let utxo_a = P256KeyPair::generate(&e);

    let nonce = 0;

    let mut deposit_op = ChannelOperationBuilder::generate(
        &e,
        channel.address.clone(),
        auth.address.clone(),
        token.address.clone(),
    );

    // no conditions as we cant properly test the G address signing mixed with the mocked address
    deposit_op.add_deposit(
        &e,
        john.address.clone(),
        500_i128,
        vec![&e, Condition::Create(utxo_a.public_key.clone(), 500_i128)],
    );

    deposit_op.add_create(utxo_a.public_key.clone(), 500_i128);

    let live_until_ledger = e.ledger().sequence() + 100;

    let provider_a_signature = provider_a.sign(
        &e,
        deposit_op.get_auth_entry_payload_hash_for_bundle(&e, nonce, live_until_ledger),
    );

    deposit_op.add_provider_signature(
        &e,
        provider_a.address.clone(),
        provider_a_signature,
        live_until_ledger,
    );

    let john_deposit_sub_signature = john.sign_for_transaction(
        &e,
        deposit_op.get_auth_entry_payload_hash_for_deposit(
            &e,
            john.address.clone(),
            nonce,
            live_until_ledger,
        ),
    );

    deposit_op.add_deposit_signature(john.address.clone(), john_deposit_sub_signature);

    // To this (prints actual value):
    std::println!(
        "Auth Entry: {:?}",
        deposit_op.get_auth_entry_for_deposit(&e, john.address.clone(), nonce, live_until_ledger,),
    );

    channel
        // .mock_all_auths()
        .set_auths(&[
            deposit_op.get_auth_entry(&e, nonce, live_until_ledger.clone()),
            deposit_op.get_auth_entry_for_deposit(
                &e,
                john.address.clone(),
                nonce,
                live_until_ledger,
            ),
        ])
        .transact(&deposit_op.get_operation_bundle());

    assert_eq!(token.balance(&john.address), 500_i128);

    assert_eq!(channel.supply(), 500_i128);

    assert_eq!(channel.utxo_balance(&utxo_a.public_key.clone()), 500_i128);
}

#[test]
fn test_auth_module() {
    let e = get_env_with_g_accounts();
    let (provider_a, provider_b, john, jane, _) = get_snapshot_g_accounts(&e);

    let (channel, auth, token, admin) = create_contracts(&e);

    assert_eq!(auth.admin(), admin.clone());
    assert_eq!(channel.auth(), auth.address);
    assert_eq!(channel.admin(), admin.clone());
    assert_eq!(channel.asset(), token.address);
    assert_eq!(channel.supply(), 0_i128);

    assert_eq!(auth.is_provider(&provider_a.address), false);
    assert_eq!(auth.is_provider(&provider_b.address), false);

    let expect_error_adding_provider_a = auth.try_add_provider(&provider_a.address);
    assert!(expect_error_adding_provider_a.is_err());

    auth.mock_auths(&[MockAuth {
        address: &admin,
        invoke: &MockAuthInvoke {
            contract: &auth.address,
            fn_name: "add_provider",
            args: (&provider_a.address,).into_val(&e),
            sub_invokes: &[],
        },
    }])
    .add_provider(&provider_a.address);

    assert_eq!(auth.is_provider(&provider_a.address), true);
    assert_eq!(auth.is_provider(&provider_b.address), false);

    auth.mock_auths(&[MockAuth {
        address: &admin,
        invoke: &MockAuthInvoke {
            contract: &auth.address,
            fn_name: "add_provider",
            args: (&provider_b.address,).into_val(&e),
            sub_invokes: &[],
        },
    }])
    .add_provider(&provider_b.address);

    assert_eq!(auth.is_provider(&provider_b.address), true);

    token
        .mock_all_auths()
        .mint(&&john.address.clone(), &1000_i128);
    token
        .mock_all_auths()
        .mint(&&jane.address.clone(), &2000_i128);
    assert_eq!(token.balance(&john.address), 1000_i128);
    assert_eq!(token.balance(&jane.address), 2000_i128);

    let utxo_a = P256KeyPair::generate(&e);
    let utxo_b = P256KeyPair::generate(&e);
    let utxo_c = P256KeyPair::generate(&e);
    let utxo_d = P256KeyPair::generate(&e);

    let mut nonce = 0;

    let mut deposit_op = ChannelOperationBuilder::generate(
        &e,
        channel.address.clone(),
        auth.address.clone(),
        token.address.clone(),
    );

    // no conditions as we cant properly test the G address signing mixed with the mocked address
    deposit_op.add_deposit(
        &e,
        john.address.clone(),
        500_i128,
        vec![
            &e,
            Condition::Create(utxo_a.public_key.clone(), 200_i128),
            Condition::Create(utxo_c.public_key.clone(), 300_i128),
        ],
    );
    deposit_op.add_deposit(
        &e,
        jane.address.clone(),
        600_i128,
        vec![
            &e,
            Condition::Create(utxo_b.public_key.clone(), 300_i128),
            Condition::Create(utxo_d.public_key.clone(), 300_i128),
        ],
    );
    deposit_op.add_create(utxo_a.public_key.clone(), 200_i128);
    deposit_op.add_create(utxo_b.public_key.clone(), 300_i128);
    deposit_op.add_create(utxo_c.public_key.clone(), 300_i128);
    deposit_op.add_create(utxo_d.public_key.clone(), 300_i128);

    let live_until_ledger = e.ledger().sequence() + 100;

    let provider_a_signature = provider_a.sign(
        &e,
        deposit_op.get_auth_entry_payload_hash_for_bundle(&e, nonce, live_until_ledger),
    );

    deposit_op.add_provider_signature(
        &e,
        provider_a.address.clone(),
        provider_a_signature,
        live_until_ledger,
    );

    let john_deposit_sub_signature = john.sign_for_transaction(
        &e,
        deposit_op.get_auth_entry_payload_hash_for_deposit(
            &e,
            john.address.clone(),
            nonce,
            live_until_ledger,
        ),
    );

    deposit_op.add_deposit_signature(john.address.clone(), john_deposit_sub_signature);

    let jane_deposit_sub_signature = jane.sign_for_transaction(
        &e,
        deposit_op.get_auth_entry_payload_hash_for_deposit(
            &e,
            jane.address.clone(),
            nonce,
            live_until_ledger,
        ),
    );

    deposit_op.add_deposit_signature(jane.address.clone(), jane_deposit_sub_signature);

    channel
        // .mock_all_auths()
        .set_auths(&[
            deposit_op.get_auth_entry(&e, nonce, live_until_ledger.clone()),
            deposit_op.get_auth_entry_for_deposit(
                &e,
                john.address.clone(),
                nonce,
                live_until_ledger,
            ),
            deposit_op.get_auth_entry_for_deposit(
                &e,
                jane.address.clone(),
                nonce,
                live_until_ledger,
            ),
        ])
        .transact(&deposit_op.get_operation_bundle());

    assert_eq!(token.balance(&john.address), 500_i128);
    assert_eq!(token.balance(&jane.address), 1400_i128);
    assert_eq!(channel.supply(), 1100_i128);

    assert_eq!(channel.utxo_balance(&utxo_a.public_key.clone()), 200_i128);
    assert_eq!(channel.utxo_balance(&utxo_b.public_key.clone()), 300_i128);
    assert_eq!(channel.utxo_balance(&utxo_c.public_key.clone()), 300_i128);
    assert_eq!(channel.utxo_balance(&utxo_d.public_key.clone()), 300_i128);

    e.ledger().set_sequence_number(3);

    let utxo_e = P256KeyPair::generate(&e);
    let utxo_f = P256KeyPair::generate(&e);
    let utxo_g = P256KeyPair::generate(&e);
    let utxo_h = P256KeyPair::generate(&e);
    let utxo_i = P256KeyPair::generate(&e);

    let mut transfer_op = ChannelOperationBuilder::generate(
        &e,
        channel.address.clone(),
        auth.address.clone(),
        token.address.clone(),
    );

    nonce = 1;

    transfer_op.add_create(utxo_e.public_key.clone(), 100_i128);
    transfer_op.add_create(utxo_f.public_key.clone(), 200_i128);
    transfer_op.add_create(utxo_g.public_key.clone(), 70_i128);
    transfer_op.add_create(utxo_h.public_key.clone(), 130_i128);
    transfer_op.add_create(utxo_i.public_key.clone(), 600_i128);

    transfer_op.add_spend(
        utxo_a.public_key.clone(),
        vec![
            &e,
            Condition::Create(utxo_e.public_key.clone(), 100_i128),
            Condition::Create(utxo_f.public_key.clone(), 200_i128),
            Condition::Create(utxo_g.public_key.clone(), 70_i128),
        ],
    );

    transfer_op.add_spend(
        utxo_b.public_key.clone(),
        vec![
            &e,
            Condition::Create(utxo_f.public_key.clone(), 200_i128),
            Condition::Create(utxo_g.public_key.clone(), 70_i128),
            Condition::Create(utxo_h.public_key.clone(), 130_i128),
        ],
    );

    transfer_op.add_spend(
        utxo_c.public_key.clone(),
        vec![
            &e,
            Condition::Create(utxo_g.public_key.clone(), 70_i128),
            Condition::Create(utxo_h.public_key.clone(), 130_i128),
            Condition::Create(utxo_i.public_key.clone(), 600_i128),
        ],
    );

    transfer_op.add_spend(
        utxo_d.public_key.clone(),
        vec![
            &e,
            Condition::Create(utxo_e.public_key.clone(), 100_i128),
            Condition::Create(utxo_g.public_key.clone(), 70_i128),
            Condition::Create(utxo_i.public_key.clone(), 600_i128),
        ],
    );

    let provider_b_signature = provider_b.sign(
        &e,
        transfer_op.get_auth_entry_payload_hash_for_bundle(&e, nonce, live_until_ledger),
    );

    transfer_op.add_provider_signature(
        &e,
        provider_b.address.clone(),
        provider_b_signature,
        live_until_ledger,
    );

    let utxo_a_signature = utxo_a.sign(&transfer_op.get_auth_hash_for_spend(
        &e,
        utxo_a.public_key.clone(),
        live_until_ledger,
    ));

    transfer_op.add_spend_signature(
        &e,
        utxo_a.public_key.clone(),
        utxo_a_signature,
        live_until_ledger,
    );

    let utxo_b_signature = utxo_b.sign(&transfer_op.get_auth_hash_for_spend(
        &e,
        utxo_b.public_key.clone(),
        live_until_ledger,
    ));
    transfer_op.add_spend_signature(
        &e,
        utxo_b.public_key.clone(),
        utxo_b_signature,
        live_until_ledger,
    );
    let utxo_c_signature = utxo_c.sign(&transfer_op.get_auth_hash_for_spend(
        &e,
        utxo_c.public_key.clone(),
        live_until_ledger,
    ));
    transfer_op.add_spend_signature(
        &e,
        utxo_c.public_key.clone(),
        utxo_c_signature,
        live_until_ledger,
    );
    let utxo_d_signature = utxo_d.sign(&transfer_op.get_auth_hash_for_spend(
        &e,
        utxo_d.public_key.clone(),
        live_until_ledger,
    ));
    transfer_op.add_spend_signature(
        &e,
        utxo_d.public_key.clone(),
        utxo_d_signature,
        live_until_ledger,
    );

    channel
        // .mock_all_auths()
        .set_auths(&[transfer_op.get_auth_entry(&e, nonce, live_until_ledger.clone())])
        .transact(&transfer_op.get_operation_bundle());

    assert_eq!(channel.supply(), 1100_i128);

    assert_eq!(channel.utxo_balance(&utxo_a.public_key.clone()), 0_i128);
    assert_eq!(channel.utxo_balance(&utxo_b.public_key.clone()), 0_i128);
    assert_eq!(channel.utxo_balance(&utxo_c.public_key.clone()), 0_i128);
    assert_eq!(channel.utxo_balance(&utxo_d.public_key.clone()), 0_i128);
    assert_eq!(channel.utxo_balance(&utxo_e.public_key.clone()), 100_i128);
    assert_eq!(channel.utxo_balance(&utxo_f.public_key.clone()), 200_i128);
    assert_eq!(channel.utxo_balance(&utxo_g.public_key.clone()), 70_i128);
    assert_eq!(channel.utxo_balance(&utxo_h.public_key.clone()), 130_i128);
    assert_eq!(channel.utxo_balance(&utxo_i.public_key.clone()), 600_i128);
}
