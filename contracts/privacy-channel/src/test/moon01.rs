#![cfg(test)]
//! MOON-01 regression tests: every owner-signed `Create` / `ExtWithdraw` condition must be executed
//! exactly (subset binding `authorized ⊆ executed`), so a signer's outputs can't be dropped,
//! reduced, or redirected. Extra executed creates/withdraws (the provider fee) are allowed but
//! bounded by the balance check to the residual the signers left. The headline tests reproduce the
//! audit's redirect attack and prove it is now REJECTED; others prove legitimate bundles
//! (multi-spend full-set, partition, withdraw, and provider-fee) are ACCEPTED, and that a provider
//! cannot take more than the residual.
extern crate std;

use crate::{
    contract::PrivacyChannelContractClient,
    test::{channel_operation_builder::ChannelOperationBuilder, test::create_contracts},
};
use channel_auth_contract::contract::ChannelAuthContractClient;
use moonlight_errors::Error as ContractError;
use moonlight_helpers::testutils::{
    keys::{Ed25519Account, P256KeyPair},
    snapshot::{get_env_with_g_accounts, get_snapshot_g_accounts},
};
use moonlight_primitives::Condition;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    vec, Address, BytesN, Env, Error, Vec,
};
use token_contract::TestTokenClient as TokenClient;

/// Fund one or more UTXOs by performing a real (provider- + depositor-signed) deposit whose
/// conditions authorize exactly the given creates. Mirrors `local-dev/lib/client/deposit.ts`.
fn fund_utxos(
    e: &Env,
    channel: &PrivacyChannelContractClient,
    auth: &ChannelAuthContractClient,
    token: &TokenClient,
    provider: &Ed25519Account,
    depositor: &Ed25519Account,
    creates: &Vec<(BytesN<65>, i128)>,
    total: i128,
    nonce: i64,
) {
    token.mock_all_auths().mint(&depositor.address, &total);

    let mut conditions: Vec<Condition> = vec![e];
    for (utxo, amount) in creates.iter() {
        conditions.push_back(Condition::Create(utxo.clone(), amount));
    }

    let mut op = ChannelOperationBuilder::generate(
        e,
        channel.address.clone(),
        auth.address.clone(),
        token.address.clone(),
    );
    op.add_deposit(e, depositor.address.clone(), total, conditions);
    for (utxo, amount) in creates.iter() {
        op.add_create(utxo.clone(), amount);
    }

    let live = e.ledger().sequence() + 100;

    let provider_sig = provider.sign(e, op.get_auth_entry_payload_hash_for_bundle(e, nonce, live));
    op.add_provider_signature(e, provider.address.clone(), provider_sig, live);

    let depositor_sig = depositor.sign_for_transaction(
        e,
        op.get_auth_entry_payload_hash_for_deposit(e, depositor.address.clone(), nonce, live),
    );
    op.add_deposit_signature(depositor.address.clone(), depositor_sig);

    channel
        .set_auths(&[
            op.get_auth_entry(e, nonce, live),
            op.get_auth_entry_for_deposit(e, depositor.address.clone(), nonce, live),
        ])
        .transact(&op.get_operation_bundle());
}

fn unauthorized(res_err: Option<Result<Error, soroban_sdk::InvokeError>>) {
    assert_eq!(
        res_err,
        Some(Ok(Error::from_contract_error(
            ContractError::UnauthorizedOperation as u32
        )))
    );
}

fn unbalanced(res_err: Option<Result<Error, soroban_sdk::InvokeError>>) {
    assert_eq!(
        res_err,
        Some(Ok(Error::from_contract_error(
            ContractError::UnbalancedBundle as u32
        )))
    );
}

/// THE GATE (audit repro). A victim signs a spend authorizing an internal change-create, but the
/// provider keeps `op.spend` byte-identical (so the P256 sig still verifies) and substitutes the
/// output with a withdrawal to an attacker — perfectly balanced. Pre-fix this was ACCEPTED and the
/// funds left the channel; post-fix it is REJECTED with `UnauthorizedOperation`.
#[test]
fn test_moon01_redirect_spend_output_to_attacker_withdraw_is_rejected() {
    let e = get_env_with_g_accounts();
    let (provider_a, _b, john, _jane, _) = get_snapshot_g_accounts(&e);
    let (channel, auth, token, _admin) = create_contracts(&e);
    auth.mock_all_auths().add_provider(&provider_a.address);

    // Fund the victim UTXO with 500.
    let utxo_victim = P256KeyPair::generate(&e);
    let creates = vec![&e, (utxo_victim.public_key.clone(), 500_i128)];
    fund_utxos(
        &e,
        &channel,
        &auth,
        &token,
        &provider_a,
        &john,
        &creates,
        500,
        0,
    );
    assert_eq!(channel.utxo_balance(&utxo_victim.public_key), 500);
    assert_eq!(channel.supply(), 500);

    e.ledger().set_sequence_number(3);
    let live = e.ledger().sequence() + 100;
    let nonce = 1;

    let attacker = Address::generate(&e);
    let utxo_change = P256KeyPair::generate(&e); // the create the victim THINKS they authorize

    let mut atk = ChannelOperationBuilder::generate(
        &e,
        channel.address.clone(),
        auth.address.clone(),
        token.address.clone(),
    );

    // Victim authorizes an INTERNAL change-create of 500 to utxo_change...
    atk.add_spend(
        utxo_victim.public_key.clone(),
        vec![
            &e,
            Condition::Create(utxo_change.public_key.clone(), 500_i128),
        ],
    );
    // ...but the bundle drops the create and redirects 500 OUT to the attacker. Balanced (500==500).
    atk.add_withdraw(&e, attacker.clone(), 500_i128, vec![&e]);

    let p_sig = provider_a.sign(
        &e,
        atk.get_auth_entry_payload_hash_for_bundle(&e, nonce, live),
    );
    atk.add_provider_signature(&e, provider_a.address.clone(), p_sig, live);

    // The victim's P256 signature is over the UNCHANGED conditions and still verifies.
    let v_sig =
        utxo_victim.sign(&atk.get_auth_hash_for_spend(&e, utxo_victim.public_key.clone(), live));
    atk.add_spend_signature(&e, utxo_victim.public_key.clone(), v_sig, live);

    let res = channel
        .set_auths(&[atk.get_auth_entry(&e, nonce, live)])
        .try_transact(&atk.get_operation_bundle());

    // POST-FIX: rejected, and no funds moved.
    unauthorized(res.err());
    assert_eq!(channel.utxo_balance(&utxo_victim.public_key), 500); // victim UTXO still unspent
    assert_eq!(token.balance(&attacker), 0); // attacker received nothing
    assert_eq!(channel.supply(), 500);
}

/// Sibling of the above: redirect the spend output to an attacker-owned CREATE (instead of a
/// withdrawal). Also rejected — the executed create is not an authorized condition.
#[test]
fn test_moon01_redirect_spend_output_to_attacker_create_is_rejected() {
    let e = get_env_with_g_accounts();
    let (provider_a, _b, john, _jane, _) = get_snapshot_g_accounts(&e);
    let (channel, auth, token, _admin) = create_contracts(&e);
    auth.mock_all_auths().add_provider(&provider_a.address);

    let utxo_victim = P256KeyPair::generate(&e);
    let creates = vec![&e, (utxo_victim.public_key.clone(), 500_i128)];
    fund_utxos(
        &e,
        &channel,
        &auth,
        &token,
        &provider_a,
        &john,
        &creates,
        500,
        0,
    );

    e.ledger().set_sequence_number(3);
    let live = e.ledger().sequence() + 100;
    let nonce = 1;

    let utxo_intended = P256KeyPair::generate(&e);
    let utxo_attacker = P256KeyPair::generate(&e);

    let mut atk = ChannelOperationBuilder::generate(
        &e,
        channel.address.clone(),
        auth.address.clone(),
        token.address.clone(),
    );
    atk.add_spend(
        utxo_victim.public_key.clone(),
        vec![
            &e,
            Condition::Create(utxo_intended.public_key.clone(), 500_i128),
        ],
    );
    // Substitute the create to an attacker-owned UTXO, same amount → balanced.
    atk.add_create(utxo_attacker.public_key.clone(), 500_i128);

    let p_sig = provider_a.sign(
        &e,
        atk.get_auth_entry_payload_hash_for_bundle(&e, nonce, live),
    );
    atk.add_provider_signature(&e, provider_a.address.clone(), p_sig, live);
    let v_sig =
        utxo_victim.sign(&atk.get_auth_hash_for_spend(&e, utxo_victim.public_key.clone(), live));
    atk.add_spend_signature(&e, utxo_victim.public_key.clone(), v_sig, live);

    let res = channel
        .set_auths(&[atk.get_auth_entry(&e, nonce, live)])
        .try_transact(&atk.get_operation_bundle());

    unauthorized(res.err());
    assert_eq!(channel.utxo_balance(&utxo_victim.public_key), 500);
    assert_eq!(channel.utxo_balance(&utxo_attacker.public_key), -1); // never created
}

/// Multi-spend legit bundle where EACH spend signs the FULL output set (the
/// `local-dev/lib/client/send.ts` convention). Set/dedup comparison must accept it.
#[test]
fn test_moon01_multispend_full_outputset_is_accepted() {
    let e = get_env_with_g_accounts();
    let (provider_a, provider_b, john, _jane, _) = get_snapshot_g_accounts(&e);
    let (channel, auth, token, _admin) = create_contracts(&e);
    auth.mock_all_auths().add_provider(&provider_a.address);
    auth.mock_all_auths().add_provider(&provider_b.address);

    // Fund two victim UTXOs (300 + 200 = 500).
    let utxo_a = P256KeyPair::generate(&e);
    let utxo_b = P256KeyPair::generate(&e);
    let creates = vec![
        &e,
        (utxo_a.public_key.clone(), 300_i128),
        (utxo_b.public_key.clone(), 200_i128),
    ];
    fund_utxos(
        &e,
        &channel,
        &auth,
        &token,
        &provider_a,
        &john,
        &creates,
        500,
        0,
    );

    e.ledger().set_sequence_number(3);
    let live = e.ledger().sequence() + 100;
    let nonce = 1;

    let utxo_x = P256KeyPair::generate(&e);
    let utxo_y = P256KeyPair::generate(&e);
    // Each spend carries the COMPLETE create set as its conditions.
    let full: Vec<Condition> = vec![
        &e,
        Condition::Create(utxo_x.public_key.clone(), 200_i128),
        Condition::Create(utxo_y.public_key.clone(), 300_i128),
    ];

    let mut op = ChannelOperationBuilder::generate(
        &e,
        channel.address.clone(),
        auth.address.clone(),
        token.address.clone(),
    );
    op.add_create(utxo_x.public_key.clone(), 200_i128);
    op.add_create(utxo_y.public_key.clone(), 300_i128);
    op.add_spend(utxo_a.public_key.clone(), full.clone());
    op.add_spend(utxo_b.public_key.clone(), full.clone());

    let p_sig = provider_b.sign(
        &e,
        op.get_auth_entry_payload_hash_for_bundle(&e, nonce, live),
    );
    op.add_provider_signature(&e, provider_b.address.clone(), p_sig, live);
    for kp in [&utxo_a, &utxo_b] {
        let sig = kp.sign(&op.get_auth_hash_for_spend(&e, kp.public_key.clone(), live));
        op.add_spend_signature(&e, kp.public_key.clone(), sig, live);
    }

    channel
        .set_auths(&[op.get_auth_entry(&e, nonce, live)])
        .transact(&op.get_operation_bundle());

    assert_eq!(channel.utxo_balance(&utxo_a.public_key), 0);
    assert_eq!(channel.utxo_balance(&utxo_b.public_key), 0);
    assert_eq!(channel.utxo_balance(&utxo_x.public_key), 200);
    assert_eq!(channel.utxo_balance(&utxo_y.public_key), 300);
    assert_eq!(channel.supply(), 500); // internal-only: supply unchanged
}

/// Legit withdraw flow (the `local-dev/lib/client/withdraw.ts` convention): the spend signs an
/// `ExtWithdraw` + a change `Create`; both execute. Must be ACCEPTED, and funds reach the
/// intended destination.
#[test]
fn test_moon01_legit_withdraw_is_accepted() {
    let e = get_env_with_g_accounts();
    let (provider_a, _b, john, _jane, _) = get_snapshot_g_accounts(&e);
    let (channel, auth, token, _admin) = create_contracts(&e);
    auth.mock_all_auths().add_provider(&provider_a.address);

    let utxo_a = P256KeyPair::generate(&e);
    let creates = vec![&e, (utxo_a.public_key.clone(), 500_i128)];
    fund_utxos(
        &e,
        &channel,
        &auth,
        &token,
        &provider_a,
        &john,
        &creates,
        500,
        0,
    );

    e.ledger().set_sequence_number(3);
    let live = e.ledger().sequence() + 100;
    let nonce = 1;

    let destination = Address::generate(&e);
    let utxo_change = P256KeyPair::generate(&e);

    let mut op = ChannelOperationBuilder::generate(
        &e,
        channel.address.clone(),
        auth.address.clone(),
        token.address.clone(),
    );
    // Spend authorizes: withdraw 400 to `destination` + keep 100 as change.
    op.add_spend(
        utxo_a.public_key.clone(),
        vec![
            &e,
            Condition::ExtWithdraw(destination.clone(), 400_i128),
            Condition::Create(utxo_change.public_key.clone(), 100_i128),
        ],
    );
    op.add_create(utxo_change.public_key.clone(), 100_i128);
    op.add_withdraw(&e, destination.clone(), 400_i128, vec![&e]);

    let p_sig = provider_a.sign(
        &e,
        op.get_auth_entry_payload_hash_for_bundle(&e, nonce, live),
    );
    op.add_provider_signature(&e, provider_a.address.clone(), p_sig, live);
    let v_sig = utxo_a.sign(&op.get_auth_hash_for_spend(&e, utxo_a.public_key.clone(), live));
    op.add_spend_signature(&e, utxo_a.public_key.clone(), v_sig, live);

    channel
        .set_auths(&[op.get_auth_entry(&e, nonce, live)])
        .transact(&op.get_operation_bundle());

    assert_eq!(token.balance(&destination), 400); // funds reached the intended recipient
    assert_eq!(channel.utxo_balance(&utxo_change.public_key), 100);
    assert_eq!(channel.utxo_balance(&utxo_a.public_key), 0);
    assert_eq!(channel.supply(), 100); // 500 deposited - 400 withdrawn
}

/// Provider fee WITHIN the residual is accepted (the real e2e shape): user spends 1000 and signs
/// `Create(dest, 995)`; the provider adds an unsigned `Create(opex, 5)` fee. Balanced 1000 == 1000.
#[test]
fn test_moon01_provider_fee_within_residual_is_accepted() {
    let e = get_env_with_g_accounts();
    let (provider_a, _b, john, _jane, _) = get_snapshot_g_accounts(&e);
    let (channel, auth, token, _admin) = create_contracts(&e);
    auth.mock_all_auths().add_provider(&provider_a.address);

    let utxo_src = P256KeyPair::generate(&e);
    let creates = vec![&e, (utxo_src.public_key.clone(), 1000_i128)];
    fund_utxos(
        &e,
        &channel,
        &auth,
        &token,
        &provider_a,
        &john,
        &creates,
        1000,
        0,
    );

    e.ledger().set_sequence_number(3);
    let live = e.ledger().sequence() + 100;
    let nonce = 1;

    let utxo_dest = P256KeyPair::generate(&e);
    let utxo_opex = P256KeyPair::generate(&e);

    let mut op = ChannelOperationBuilder::generate(
        &e,
        channel.address.clone(),
        auth.address.clone(),
        token.address.clone(),
    );
    op.add_spend(
        utxo_src.public_key.clone(),
        vec![
            &e,
            Condition::Create(utxo_dest.public_key.clone(), 995_i128),
        ], // signer claims 995
    );
    op.add_create(utxo_dest.public_key.clone(), 995_i128);
    op.add_create(utxo_opex.public_key.clone(), 5_i128); // provider fee (unsigned), == residual

    let p_sig = provider_a.sign(
        &e,
        op.get_auth_entry_payload_hash_for_bundle(&e, nonce, live),
    );
    op.add_provider_signature(&e, provider_a.address.clone(), p_sig, live);
    let s_sig = utxo_src.sign(&op.get_auth_hash_for_spend(&e, utxo_src.public_key.clone(), live));
    op.add_spend_signature(&e, utxo_src.public_key.clone(), s_sig, live);

    channel
        .set_auths(&[op.get_auth_entry(&e, nonce, live)])
        .transact(&op.get_operation_bundle());

    assert_eq!(channel.utxo_balance(&utxo_src.public_key), 0);
    assert_eq!(channel.utxo_balance(&utxo_dest.public_key), 995); // signer's output delivered exactly
    assert_eq!(channel.utxo_balance(&utxo_opex.public_key), 5); // provider fee
}

/// Provider tries to take MORE than the residual: user signs `Create(dest, 995)`, provider adds
/// `Create(opex, 50)` (residual is only 5). Balance check rejects (Σcreated 1045 != Σspent 1000).
#[test]
fn test_moon01_provider_fee_exceeding_residual_is_rejected() {
    let e = get_env_with_g_accounts();
    let (provider_a, _b, john, _jane, _) = get_snapshot_g_accounts(&e);
    let (channel, auth, token, _admin) = create_contracts(&e);
    auth.mock_all_auths().add_provider(&provider_a.address);

    let utxo_src = P256KeyPair::generate(&e);
    let creates = vec![&e, (utxo_src.public_key.clone(), 1000_i128)];
    fund_utxos(
        &e,
        &channel,
        &auth,
        &token,
        &provider_a,
        &john,
        &creates,
        1000,
        0,
    );

    e.ledger().set_sequence_number(3);
    let live = e.ledger().sequence() + 100;
    let nonce = 1;

    let utxo_dest = P256KeyPair::generate(&e);
    let utxo_opex = P256KeyPair::generate(&e);

    let mut op = ChannelOperationBuilder::generate(
        &e,
        channel.address.clone(),
        auth.address.clone(),
        token.address.clone(),
    );
    op.add_spend(
        utxo_src.public_key.clone(),
        vec![
            &e,
            Condition::Create(utxo_dest.public_key.clone(), 995_i128),
        ],
    );
    op.add_create(utxo_dest.public_key.clone(), 995_i128);
    op.add_create(utxo_opex.public_key.clone(), 50_i128); // exceeds the 5 residual

    let p_sig = provider_a.sign(
        &e,
        op.get_auth_entry_payload_hash_for_bundle(&e, nonce, live),
    );
    op.add_provider_signature(&e, provider_a.address.clone(), p_sig, live);
    let s_sig = utxo_src.sign(&op.get_auth_hash_for_spend(&e, utxo_src.public_key.clone(), live));
    op.add_spend_signature(&e, utxo_src.public_key.clone(), s_sig, live);

    let res = channel
        .set_auths(&[op.get_auth_entry(&e, nonce, live)])
        .try_transact(&op.get_operation_bundle());

    unbalanced(res.err());
    assert_eq!(channel.utxo_balance(&utxo_src.public_key), 1000); // nothing moved
}

/// Pure internal transfer (residual 0): user spends 500 and signs `Create(dest, 500)`. A provider
/// attempt to add ANY extra create has no residual to fund it -> balance check rejects.
#[test]
fn test_moon01_internal_transfer_extra_create_is_rejected() {
    let e = get_env_with_g_accounts();
    let (provider_a, _b, john, _jane, _) = get_snapshot_g_accounts(&e);
    let (channel, auth, token, _admin) = create_contracts(&e);
    auth.mock_all_auths().add_provider(&provider_a.address);

    let utxo_src = P256KeyPair::generate(&e);
    let creates = vec![&e, (utxo_src.public_key.clone(), 500_i128)];
    fund_utxos(
        &e,
        &channel,
        &auth,
        &token,
        &provider_a,
        &john,
        &creates,
        500,
        0,
    );

    e.ledger().set_sequence_number(3);
    let live = e.ledger().sequence() + 100;
    let nonce = 1;

    let utxo_dest = P256KeyPair::generate(&e);
    let utxo_opex = P256KeyPair::generate(&e);

    let mut op = ChannelOperationBuilder::generate(
        &e,
        channel.address.clone(),
        auth.address.clone(),
        token.address.clone(),
    );
    op.add_spend(
        utxo_src.public_key.clone(),
        vec![
            &e,
            Condition::Create(utxo_dest.public_key.clone(), 500_i128),
        ], // claims all 500
    );
    op.add_create(utxo_dest.public_key.clone(), 500_i128);
    op.add_create(utxo_opex.public_key.clone(), 1_i128); // no residual to fund this

    let p_sig = provider_a.sign(
        &e,
        op.get_auth_entry_payload_hash_for_bundle(&e, nonce, live),
    );
    op.add_provider_signature(&e, provider_a.address.clone(), p_sig, live);
    let s_sig = utxo_src.sign(&op.get_auth_hash_for_spend(&e, utxo_src.public_key.clone(), live));
    op.add_spend_signature(&e, utxo_src.public_key.clone(), s_sig, live);

    let res = channel
        .set_auths(&[op.get_auth_entry(&e, nonce, live)])
        .try_transact(&op.get_operation_bundle());

    unbalanced(res.err());
    assert_eq!(channel.utxo_balance(&utxo_src.public_key), 500);
}
