use crate::*;
use soroban_sdk::{testutils::Address as _, vec, Address, Env};

fn contract_bytes(e: &Env) -> Bytes {
    Address::generate(e).to_string().to_bytes()
}

/// A1: a deposit and a withdraw over the same (address, amount) previously
/// concatenated into separate buckets and hashed to byte-identical payloads,
/// letting a signed deposit be reinterpreted as a withdraw at tx-build time.
/// The XDR encoding tags the variant, so the two now hash differently.
#[test]
fn deposit_and_withdraw_over_same_address_amount_hash_differently() {
    let e = Env::default();
    let contract = contract_bytes(&e);
    let addr = Address::generate(&e);
    let amount: i128 = 1_000;

    let deposit = AuthPayload {
        conditions: vec![&e, Condition::ExtDeposit(addr.clone(), amount)],
        live_until_ledger: 100,
    };
    let withdraw = AuthPayload {
        conditions: vec![&e, Condition::ExtWithdraw(addr.clone(), amount)],
        live_until_ledger: 100,
    };

    assert_ne!(
        hash_payload(&e, &deposit, &contract).to_bytes(),
        hash_payload(&e, &withdraw, &contract).to_bytes(),
        "Dep(X,a) and Wd(X,a) must not share a signed digest"
    );
}

/// A cross-type reordering the old bucket layout ignored (deposit and withdraw
/// land in fixed buckets regardless of input order). XDR preserves list order,
/// so the digest now binds exactly the ordering the signer reviewed.
#[test]
fn reordering_conditions_changes_the_hash() {
    let e = Env::default();
    let contract = contract_bytes(&e);
    let a = Address::generate(&e);
    let b = Address::generate(&e);

    let dep = Condition::ExtDeposit(a, 10);
    let wd = Condition::ExtWithdraw(b, 20);

    let forward = AuthPayload {
        conditions: vec![&e, dep.clone(), wd.clone()],
        live_until_ledger: 100,
    };
    let reversed = AuthPayload {
        conditions: vec![&e, wd, dep],
        live_until_ledger: 100,
    };

    assert_ne!(
        hash_payload(&e, &forward, &contract).to_bytes(),
        hash_payload(&e, &reversed, &contract).to_bytes(),
        "condition ordering must be bound by the digest"
    );
}

/// The encoding stays deterministic: identical payloads hash identically, so
/// the existing signature-verification path is unaffected.
#[test]
fn identical_payloads_hash_identically() {
    let e = Env::default();
    let contract = contract_bytes(&e);
    let addr = Address::generate(&e);

    let build = || AuthPayload {
        conditions: vec![
            &e,
            Condition::ExtDeposit(addr.clone(), 42),
            Condition::Create(BytesN::from_array(&e, &[7u8; 65]), 99),
        ],
        live_until_ledger: 555,
    };

    assert_eq!(
        hash_payload(&e, &build(), &contract).to_bytes(),
        hash_payload(&e, &build(), &contract).to_bytes(),
    );
}
