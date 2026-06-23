use soroban_sdk::testutils::storage::Persistent as _;
use soroban_sdk::testutils::Ledger as _;
use soroban_sdk::{contract, Address, BytesN, Env};

use crate::{hash_utxo_key, Store, UTXOCoreDataKey};

#[contract]
struct StorageTestContract;

fn storage_contract(e: &Env) -> Address {
    e.register(StorageTestContract, ())
}

fn in_contract<R>(e: &Env, contract_id: &Address, f: impl FnOnce() -> R) -> R {
    e.as_contract(contract_id, f)
}

fn utxo(e: &Env, seed: u8) -> BytesN<65> {
    let mut bytes = [0u8; 65];
    bytes[0] = 4;

    for i in 1..65 {
        bytes[i] = seed.wrapping_add(i as u8);
    }

    BytesN::<65>::from_array(e, &bytes)
}

/// Reads the raw per-UTXO spend-state entry (the stored amount, or `None` if no record exists).
fn spend_state(e: &Env, utxo65: &BytesN<65>) -> Option<i128> {
    e.storage()
        .persistent()
        .get::<_, i128>(&UTXOCoreDataKey::UTXO(hash_utxo_key(e, utxo65)))
}

fn utxo_key(e: &Env, utxo65: &BytesN<65>) -> UTXOCoreDataKey {
    UTXOCoreDataKey::UTXO(hash_utxo_key(e, utxo65))
}

#[test]
fn missing_utxo_balance_is_negative_one_without_creating_an_entry() {
    let e = Env::default();
    let contract_id = storage_contract(&e);

    in_contract(&e, &contract_id, || {
        let missing = utxo(&e, 1);

        let balance = Store::apply(&e, |store| store.balance(&missing));

        assert_eq!(balance, -1);
        assert!(spend_state(&e, &missing).is_none());
    });
}

#[test]
fn create_persists_the_amount_in_a_per_utxo_entry() {
    let e = Env::default();
    let contract_id = storage_contract(&e);

    in_contract(&e, &contract_id, || {
        let first = utxo(&e, 1);

        Store::apply(&e, |store| store.create(&first, 100));

        assert_eq!(Store::apply(&e, |store| store.balance(&first)), 100);
        assert_eq!(spend_state(&e, &first), Some(100));
    });
}

#[test]
fn each_created_utxo_gets_its_own_independent_entry() {
    let e = Env::default();
    let contract_id = storage_contract(&e);

    in_contract(&e, &contract_id, || {
        Store::apply(&e, |store| {
            for seed in 1..=9 {
                store.create(&utxo(&e, seed), seed as i128);
            }
        });

        for seed in 1..=9 {
            assert_eq!(spend_state(&e, &utxo(&e, seed)), Some(seed as i128));
            assert_eq!(
                Store::apply(&e, |store| store.balance(&utxo(&e, seed))),
                seed as i128
            );
        }
    });
}

#[test]
fn spend_tombstones_only_the_target_and_leaves_other_entries_unspent() {
    let e = Env::default();
    let contract_id = storage_contract(&e);

    in_contract(&e, &contract_id, || {
        let first = utxo(&e, 1);
        let second = utxo(&e, 2);
        let third = utxo(&e, 3);

        Store::apply(&e, |store| {
            store.create(&first, 10);
            store.create(&second, 20);
            store.create(&third, 30);
        });

        Store::apply(&e, |store| {
            assert_eq!(store.spend(&first), 10);
            assert_eq!(store.spend(&third), 30);
        });

        Store::apply(&e, |store| {
            assert_eq!(store.balance(&first), 0);
            assert_eq!(store.balance(&second), 20);
            assert_eq!(store.balance(&third), 0);
        });

        // Spent UTXOs are tombstoned to 0; the untouched one keeps its amount.
        assert_eq!(spend_state(&e, &first), Some(0));
        assert_eq!(spend_state(&e, &second), Some(20));
        assert_eq!(spend_state(&e, &third), Some(0));
    });
}

#[test]
fn created_utxo_can_be_read_and_spent_inside_the_same_scope() {
    let e = Env::default();
    let contract_id = storage_contract(&e);

    in_contract(&e, &contract_id, || {
        let key = utxo(&e, 1);

        Store::apply(&e, |store| {
            store.create(&key, 44);
            assert_eq!(store.balance(&key), 44);
            assert_eq!(store.spend(&key), 44);
            assert_eq!(store.balance(&key), 0);
        });

        assert_eq!(Store::apply(&e, |store| store.balance(&key)), 0);
    });
}

#[test]
fn create_spend_and_balance_bump_persistent_ttl() {
    // MOON-02: the per-UTXO spend-state entry must be pushed to the long (30-day) TTL window so it
    // cannot archive while the channel is live. Each UTXO's liveness is independent: create, spend,
    // and even a plain balance read by the holder refresh its own entry's TTL.
    let e = Env::default();
    let contract_id = storage_contract(&e);

    in_contract(&e, &contract_id, || {
        let key = utxo(&e, 1);
        let uk = utxo_key(&e, &key);
        let min_ttl = Store::PERSISTENT_BUMP_AMOUNT - Store::DAY_IN_LEDGERS;

        Store::apply(&e, |store| store.create(&key, 100));
        assert!(e.storage().persistent().get_ttl(&uk) >= min_ttl);

        // A balance read alone keeps the holder's entry alive.
        e.ledger().with_mut(|l| l.sequence_number += 1);
        Store::apply(&e, |store| store.balance(&key));
        assert!(e.storage().persistent().get_ttl(&uk) >= min_ttl);

        // Spending keeps the now-spent tombstone alive (so it cannot be recreated post-archival).
        Store::apply(&e, |store| {
            store.spend(&key);
        });
        assert_eq!(spend_state(&e, &key), Some(0));
        assert!(e.storage().persistent().get_ttl(&uk) >= min_ttl);
    });
}

#[test]
#[should_panic]
fn create_rejects_duplicate_utxo_even_after_spend() {
    let e = Env::default();
    let contract_id = storage_contract(&e);

    in_contract(&e, &contract_id, || {
        let key = utxo(&e, 1);

        Store::apply(&e, |store| {
            store.create(&key, 100);
            store.spend(&key);
        });

        // The spent tombstone blocks recreation.
        Store::apply(&e, |store| store.create(&key, 100));
    });
}

#[test]
#[should_panic]
fn create_rejects_zero_amount() {
    let e = Env::default();
    let contract_id = storage_contract(&e);

    in_contract(&e, &contract_id, || {
        let key = utxo(&e, 1);
        Store::apply(&e, |store| store.create(&key, 0));
    });
}

#[test]
#[should_panic]
fn create_rejects_negative_amount() {
    let e = Env::default();
    let contract_id = storage_contract(&e);

    in_contract(&e, &contract_id, || {
        let key = utxo(&e, 1);
        Store::apply(&e, |store| store.create(&key, -1));
    });
}

#[test]
#[should_panic]
fn spend_rejects_missing_utxo() {
    let e = Env::default();
    let contract_id = storage_contract(&e);

    in_contract(&e, &contract_id, || {
        let key = utxo(&e, 1);
        Store::apply(&e, |store| {
            store.spend(&key);
        });
    });
}

#[test]
#[should_panic]
fn spend_rejects_already_spent_utxo() {
    let e = Env::default();
    let contract_id = storage_contract(&e);

    in_contract(&e, &contract_id, || {
        let key = utxo(&e, 1);
        Store::apply(&e, |store| {
            store.create(&key, 100);
            store.spend(&key);
            store.spend(&key);
        });
    });
}
