/* =========================
Simple storage
========================= */

use soroban_sdk::{panic_with_error, BytesN, Env};

use crate::{Error, UTXOCoreDataKey, UtxoState, UtxoStore};

pub struct SimpleStore;

impl SimpleStore {
    fn key(e: &Env, utxo65: &BytesN<65>) -> UTXOCoreDataKey {
        UTXOCoreDataKey::UTXO(<SimpleStore as UtxoStore>::hash_utxo_key(e, utxo65))
    }
}

impl UtxoStore for SimpleStore {
    fn utxo_balance(e: &Env, utxo65: BytesN<65>) -> i128 {
        match e
            .storage()
            .persistent()
            .get::<_, UtxoState>(&Self::key(e, &utxo65))
        {
            Some(UtxoState::Unspent(a)) => a,
            Some(UtxoState::Spent) => 0,
            None => -1,
        }
    }

    fn create(e: &Env, utxo65: BytesN<65>, amount: i128) {
        let k = Self::key(e, &utxo65);
        if e.storage().persistent().get::<_, UtxoState>(&k).is_some() {
            panic_with_error!(e, Error::UTXOAlreadyExists);
        }
        e.storage()
            .persistent()
            .set(&k, &UtxoState::Unspent(amount));
    }

    fn spend(e: &Env, utxo65: BytesN<65>) -> i128 {
        let k = Self::key(e, &utxo65);
        match e.storage().persistent().get::<_, UtxoState>(&k) {
            Some(UtxoState::Unspent(a)) => {
                e.storage().persistent().set(&k, &UtxoState::Spent);
                a
            }
            Some(UtxoState::Spent) => panic_with_error!(e, Error::UTXOAlreadySpent),
            None => panic_with_error!(e, Error::UTXODoesntExist),
        }
    }
}
