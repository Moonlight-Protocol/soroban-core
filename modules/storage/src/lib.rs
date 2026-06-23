#![no_std]

use soroban_sdk::{contracttype, panic_with_error, Bytes, BytesN, Env};

pub use moonlight_errors::Error;

#[cfg(test)]
mod test;

/// Persistent storage key for a UTXO's spend state.
#[derive(Clone)]
#[contracttype]
pub enum UTXOCoreDataKey {
    /// A 32-byte hash of a 65-byte UTXO public key.
    UTXO(BytesN<32>),
}

/// Per-UTXO spend-state storage.
///
/// Each UTXO owns a single persistent entry keyed by `hash(utxo65)` whose value
/// is the UTXO's amount: a positive value means unspent, `0` means spent, and an
/// absent entry means no record exists. No state is shared between UTXOs, so one
/// UTXO's liveness never depends on another's.
///
/// Use [`Store::apply`] to run one logical group of storage reads and writes.
pub struct Store {
    env: Env,
}

impl Store {
    // MOON-02: persistent-entry TTL management. A UTXO's spend-state entry backs user funds and
    // must outlive long idle periods; without an explicit bump it would archive. Because each UTXO
    // has its own entry, a holder keeps their own UTXO alive independently of everyone else.
    const DAY_IN_LEDGERS: u32 = 17_280;
    const PERSISTENT_BUMP_AMOUNT: u32 = 30 * Self::DAY_IN_LEDGERS;
    const PERSISTENT_LIFETIME_THRESHOLD: u32 =
        Self::PERSISTENT_BUMP_AMOUNT - Self::DAY_IN_LEDGERS;

    /// Runs UTXO storage operations in a scoped store.
    ///
    /// The closure receives the only mutable handle to storage operations. Each
    /// `create`/`spend`/`balance` writes its own per-UTXO entry directly, so
    /// there is no deferred commit step. If the closure panics, the invocation
    /// aborts before any further writes.
    ///
    /// # Panics
    ///
    /// Panics if the closure panics.
    pub fn apply<R>(e: &Env, f: impl FnOnce(&mut Store) -> R) -> R {
        let mut store = Self { env: e.clone() };

        f(&mut store)
    }

    /// Returns the balance state for a UTXO key.
    ///
    /// A positive value means the UTXO is unspent with that amount, `0` means
    /// the UTXO exists but has already been spent, and `-1` means no UTXO record
    /// exists for the key.
    ///
    /// Reading an existing entry refreshes its TTL (MOON-02), so a holder keeps
    /// their own UTXO alive simply by observing it.
    pub fn balance(&mut self, utxo65: &BytesN<65>) -> i128 {
        let k = self.utxo_key(utxo65);
        match self.env.storage().persistent().get::<_, i128>(&k) {
            Some(amount) => {
                self.bump_ttl(&k);
                amount
            }
            None => -1,
        }
    }

    /// Creates a new unspent UTXO with the provided amount.
    ///
    /// # Panics
    ///
    /// Panics if the amount is not positive or if a record already exists for
    /// the UTXO key (including a spent record, which can never be recreated).
    pub fn create(&mut self, utxo65: &BytesN<65>, amount: i128) {
        if amount <= 0 {
            panic_with_error!(&self.env, Error::InvalidCreateAmount);
        }

        let k = self.utxo_key(utxo65);

        if self.env.storage().persistent().get::<_, i128>(&k).is_some() {
            panic_with_error!(&self.env, Error::UtxoAlreadyExists);
        }

        self.env.storage().persistent().set(&k, &amount);
        self.bump_ttl(&k);
    }

    /// Spends an existing unspent UTXO and returns its amount.
    ///
    /// The entry is tombstoned in place (amount set to `0`) rather than removed,
    /// so the spent record keeps blocking re-spend and re-creation. Its TTL is
    /// refreshed so the tombstone survives long idle periods (MOON-02).
    ///
    /// # Panics
    ///
    /// Panics if the UTXO does not exist or was already spent.
    pub fn spend(&mut self, utxo65: &BytesN<65>) -> i128 {
        let k = self.utxo_key(utxo65);
        match self.env.storage().persistent().get::<_, i128>(&k) {
            Some(amount) if amount > 0 => {
                self.env.storage().persistent().set(&k, &0i128);
                // Keep the spent record alive so the UTXO cannot be recreated after archival.
                self.bump_ttl(&k);
                amount
            }
            Some(_) => panic_with_error!(&self.env, Error::UtxoAlreadySpent),
            None => panic_with_error!(&self.env, Error::UtxoDoesNotExist),
        }
    }

    #[inline(always)]
    fn bump_ttl(&self, key: &UTXOCoreDataKey) {
        self.env.storage().persistent().extend_ttl(
            key,
            Self::PERSISTENT_LIFETIME_THRESHOLD,
            Self::PERSISTENT_BUMP_AMOUNT,
        );
    }

    #[inline(always)]
    fn utxo_key(&self, utxo65: &BytesN<65>) -> UTXOCoreDataKey {
        UTXOCoreDataKey::UTXO(hash_utxo_key(&self.env, utxo65))
    }
}

fn hash_utxo_key(e: &Env, utxo65: &BytesN<65>) -> BytesN<32> {
    let b = Bytes::from_slice(e, utxo65.to_array().as_ref());
    let h = e.crypto().sha256(&b);
    BytesN::<32>::from_array(e, &h.to_array())
}
