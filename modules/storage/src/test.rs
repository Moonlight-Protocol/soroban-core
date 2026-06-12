use soroban_sdk::testutils::storage::Persistent as _;
use soroban_sdk::{contract, Address, Bytes, BytesN, Env};

use crate::{hash_utxo_key, DrawerDataKey, DrawerState, Store, UTXOCoreDataKey, UtxoMeta};

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

fn metadata(e: &Env, utxo65: &BytesN<65>) -> UtxoMeta {
    e.storage()
        .persistent()
        .get::<_, UtxoMeta>(&UTXOCoreDataKey::UTXO(hash_utxo_key(e, utxo65)))
        .unwrap()
}

fn drawer_state(e: &Env) -> DrawerState {
    e.storage()
        .persistent()
        .get::<_, DrawerState>(&DrawerDataKey::State)
        .unwrap()
}

fn drawer_bitmap(e: &Env, drawer_id: u32) -> Bytes {
    e.storage()
        .persistent()
        .get::<_, Bytes>(&Store::drawer_key(drawer_id))
        .unwrap()
}

#[test]
fn missing_utxo_balance_is_negative_one_without_allocating_drawer_state() {
    let e = Env::default();
    let contract_id = storage_contract(&e);

    in_contract(&e, &contract_id, || {
        let missing = utxo(&e, 1);

        let balance = Store::apply(&e, |store| store.balance(&missing));

        assert_eq!(balance, -1);
        assert!(e
            .storage()
            .persistent()
            .get::<_, DrawerState>(&DrawerDataKey::State)
            .is_none());
    });
}

#[test]
fn create_persists_metadata_and_sets_first_drawer_bit() {
    let e = Env::default();
    let contract_id = storage_contract(&e);

    in_contract(&e, &contract_id, || {
        let first = utxo(&e, 1);

        Store::apply(&e, |store| store.create(&first, 100));

        assert_eq!(Store::apply(&e, |store| store.balance(&first)), 100);

        let meta = metadata(&e, &first);
        assert_eq!(meta.amount, 100);
        assert_eq!(meta.drawer_id, 1);
        assert_eq!(meta.slot_idx, 0);

        let state = drawer_state(&e);
        assert_eq!(state.current_drawer, 1);
        assert_eq!(state.next_slot, 1);

        let bitmap = drawer_bitmap(&e, 1);
        assert_eq!(bitmap.len(), 1);
        assert_eq!(bitmap.get(0), Some(0b0000_0001));
    });
}

#[test]
fn creates_pack_status_bits_into_the_same_drawer_bitmap() {
    let e = Env::default();
    let contract_id = storage_contract(&e);

    in_contract(&e, &contract_id, || {
        Store::apply(&e, |store| {
            for seed in 1..=9 {
                store.create(&utxo(&e, seed), seed as i128);
            }
        });

        let bitmap = drawer_bitmap(&e, 1);
        assert_eq!(bitmap.len(), 2);
        assert_eq!(bitmap.get(0), Some(0b1111_1111));
        assert_eq!(bitmap.get(1), Some(0b0000_0001));

        let state = drawer_state(&e);
        assert_eq!(state.current_drawer, 1);
        assert_eq!(state.next_slot, 9);

        for seed in 1..=9 {
            let meta = metadata(&e, &utxo(&e, seed));
            assert_eq!(meta.drawer_id, 1);
            assert_eq!(meta.slot_idx, u32::from(seed - 1));
        }
    });
}

#[test]
fn spend_clears_only_the_target_bits_and_preserves_metadata() {
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

        let bitmap = drawer_bitmap(&e, 1);
        assert_eq!(bitmap.get(0), Some(0b0000_0010));

        let spent_meta = metadata(&e, &first);
        assert_eq!(spent_meta.amount, 10);
        assert_eq!(spent_meta.drawer_id, 1);
        assert_eq!(spent_meta.slot_idx, 0);
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
fn rotates_to_the_next_drawer_when_the_current_drawer_is_full() {
    let e = Env::default();
    let contract_id = storage_contract(&e);

    in_contract(&e, &contract_id, || {
        let key = utxo(&e, 1);

        e.storage().persistent().set(
            &DrawerDataKey::State,
            &DrawerState {
                current_drawer: 1,
                next_slot: Store::SLOTS_PER_DRAWER,
            },
        );

        Store::apply(&e, |store| store.create(&key, 100));

        let meta = metadata(&e, &key);
        assert_eq!(meta.drawer_id, 2);
        assert_eq!(meta.slot_idx, 0);

        let state = drawer_state(&e);
        assert_eq!(state.current_drawer, 2);
        assert_eq!(state.next_slot, 1);

        let bitmap = drawer_bitmap(&e, 2);
        assert_eq!(bitmap.len(), 1);
        assert_eq!(bitmap.get(0), Some(0b0000_0001));
    });
}

#[test]
fn create_and_spend_bump_persistent_ttl() {
    // MOON-02: UTXO metadata, the shared drawer bitmap, and the allocation-state entry must be
    // pushed to the long (30-day) TTL window so they cannot archive while the channel is live.
    let e = Env::default();
    let contract_id = storage_contract(&e);

    in_contract(&e, &contract_id, || {
        let key = utxo(&e, 1);
        let uk = UTXOCoreDataKey::UTXO(hash_utxo_key(&e, &key));

        Store::apply(&e, |store| store.create(&key, 100));

        let min_ttl = Store::PERSISTENT_BUMP_AMOUNT - Store::DAY_IN_LEDGERS;
        assert!(e.storage().persistent().get_ttl(&uk) >= min_ttl);
        assert!(e.storage().persistent().get_ttl(&Store::drawer_key(1)) >= min_ttl);
        assert!(e.storage().persistent().get_ttl(&DrawerDataKey::State) >= min_ttl);

        // Spending keeps the now-spent record alive (so it cannot be recreated post-archival).
        Store::apply(&e, |store| {
            store.spend(&key);
        });
        assert!(e.storage().persistent().get_ttl(&uk) >= min_ttl);
    });
}

#[test]
#[should_panic]
fn bitmap_byte_index_rejects_slot_outside_drawer() {
    let e = Env::default();
    let contract_id = storage_contract(&e);

    in_contract(&e, &contract_id, || {
        Store::apply(&e, |store| {
            store.bitmap_byte_index(Store::SLOTS_PER_DRAWER);
        });
    });
}

#[test]
#[should_panic]
fn balance_rejects_utxo_metadata_with_slot_outside_drawer() {
    let e = Env::default();
    let contract_id = storage_contract(&e);

    in_contract(&e, &contract_id, || {
        let key = utxo(&e, 1);
        let uk = UTXOCoreDataKey::UTXO(hash_utxo_key(&e, &key));

        e.storage().persistent().set(
            &uk,
            &UtxoMeta {
                amount: 100,
                drawer_id: 1,
                slot_idx: Store::SLOTS_PER_DRAWER,
            },
        );

        Store::apply(&e, |store| store.balance(&key));
    });
}

#[test]
#[should_panic]
fn spend_rejects_utxo_metadata_with_slot_outside_drawer() {
    let e = Env::default();
    let contract_id = storage_contract(&e);

    in_contract(&e, &contract_id, || {
        let key = utxo(&e, 1);
        let uk = UTXOCoreDataKey::UTXO(hash_utxo_key(&e, &key));

        e.storage().persistent().set(
            &uk,
            &UtxoMeta {
                amount: 100,
                drawer_id: 1,
                slot_idx: Store::SLOTS_PER_DRAWER,
            },
        );

        Store::apply(&e, |store| store.spend(&key));
    });
}

#[test]
#[should_panic]
fn create_rejects_duplicate_utxo_metadata_even_after_spend() {
    let e = Env::default();
    let contract_id = storage_contract(&e);

    in_contract(&e, &contract_id, || {
        let key = utxo(&e, 1);

        Store::apply(&e, |store| {
            store.create(&key, 100);
            store.spend(&key);
        });

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
