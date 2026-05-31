#![no_std]

mod cache;

use cache::DrawerCache;
use soroban_sdk::{contracttype, panic_with_error, Bytes, BytesN, Env};

pub use moonlight_errors::Error;

#[cfg(test)]
mod test;

/// Persistent storage key for UTXO metadata.
#[derive(Clone)]
#[contracttype]
pub enum UTXOCoreDataKey {
    /// A 32-byte hash of a 65-byte UTXO public key.
    UTXO(BytesN<32>),
}

/// Persistent storage key for a drawer bitmap.
#[derive(Clone)]
#[contracttype]
pub struct DrawerKey {
    /// Sequential drawer identifier.
    pub id: u32,
}

#[derive(Clone)]
#[contracttype]
struct UtxoMeta {
    amount: i128,
    drawer_id: u32,
    slot_idx: u32,
}

#[derive(Clone)]
#[contracttype]
struct DrawerState {
    current_drawer: u32,
    next_slot: u32,
}

#[derive(Clone)]
#[contracttype]
enum DrawerDataKey {
    Drawer(DrawerKey),
    State,
}

/// Drawer-backed UTXO storage.
///
/// Use [`Store::apply`] to run one logical group of storage reads and writes.
/// The store keeps drawer state and bitmap updates cached for the duration of
/// the closure, then commits dirty drawer entries once the closure returns.
pub struct Store {
    env: Env,
    cache: DrawerCache,
}

impl Store {
    // Each drawer bitmap tracks 512 Ki UTXO slots, giving each drawer a maximum
    // 64 KiB bitmap. The value grows lazily as slots are allocated, so early
    // drawers do not pay the full 64 KiB allocation cost upfront.
    const SLOTS_PER_DRAWER: u32 = 524_288;

    // One bitmap byte tracks 8 slots, rounded up if the drawer size changes.
    const BITMAP_BYTES: u32 = (Self::SLOTS_PER_DRAWER + 7) / 8;

    /// Runs UTXO storage operations in a scoped drawer cache.
    ///
    /// The closure receives the only mutable handle to storage operations. Any
    /// dirty drawer state or bitmap changes are committed after the closure
    /// returns. If the closure panics, the invocation aborts and the commit step
    /// is not reached.
    ///
    /// # Panics
    ///
    /// Panics if the closure panics.
    pub fn apply<R>(e: &Env, f: impl FnOnce(&mut Store) -> R) -> R {
        let mut store = Self {
            env: e.clone(),
            cache: DrawerCache::new(e),
        };

        let result = f(&mut store);

        store.cache.commit(e);

        result
    }

    /// Returns the balance state for a UTXO key.
    ///
    /// A positive value means the UTXO is unspent with that amount, `0` means
    /// the UTXO exists but has already been spent, and `-1` means no UTXO record
    /// exists for the key.
    pub fn balance(&mut self, utxo65: &BytesN<65>) -> i128 {
        let k = self.utxo_key(utxo65);
        match self.env.storage().persistent().get::<_, UtxoMeta>(&k) {
            Some(UtxoMeta {
                amount,
                drawer_id,
                slot_idx,
            }) => {
                let bitmap = self.get_or_create_bitmap(drawer_id);
                if self.is_bit_set(&bitmap, slot_idx) {
                    amount
                } else {
                    0
                }
            }
            None => -1,
        }
    }

    /// Creates a new unspent UTXO with the provided amount.
    ///
    /// # Panics
    ///
    /// Panics if the amount is not positive or if a record already exists for
    /// the UTXO key.
    pub fn create(&mut self, utxo65: &BytesN<65>, amount: i128) {
        if amount <= 0 {
            panic_with_error!(&self.env, Error::InvalidCreateAmount);
        }

        let uk = self.utxo_key(utxo65);

        if self
            .env
            .storage()
            .persistent()
            .get::<_, UtxoMeta>(&uk)
            .is_some()
        {
            panic_with_error!(&self.env, Error::UtxoAlreadyExists);
        }

        let (drawer_id, slot_idx) = self.alloc_slot_and_rotate_if_needed();

        self.env.storage().persistent().set(
            &uk,
            &UtxoMeta {
                amount,
                drawer_id,
                slot_idx,
            },
        );

        let mut bitmap = self.get_or_create_bitmap(drawer_id);
        self.set_bit_in_bitmap(&mut bitmap, slot_idx, true);
        self.cache.drawers.set(drawer_id, bitmap);
        self.cache.dirty_drawers.set(drawer_id, true);
    }

    /// Spends an existing unspent UTXO and returns its amount.
    ///
    /// # Panics
    ///
    /// Panics if the UTXO does not exist or was already spent.
    pub fn spend(&mut self, utxo65: &BytesN<65>) -> i128 {
        let uk = self.utxo_key(utxo65);
        match self.env.storage().persistent().get::<_, UtxoMeta>(&uk) {
            Some(UtxoMeta {
                amount,
                drawer_id,
                slot_idx,
            }) => {
                let mut bitmap = self.get_or_create_bitmap(drawer_id);

                if !self.is_bit_set(&bitmap, slot_idx) {
                    panic_with_error!(&self.env, Error::UtxoAlreadySpent);
                }

                self.set_bit_in_bitmap(&mut bitmap, slot_idx, false);
                self.cache.drawers.set(drawer_id, bitmap);
                self.cache.dirty_drawers.set(drawer_id, true);

                amount
            }
            None => panic_with_error!(&self.env, Error::UtxoDoesNotExist),
        }
    }

    #[inline(always)]
    fn utxo_key(&self, utxo65: &BytesN<65>) -> UTXOCoreDataKey {
        UTXOCoreDataKey::UTXO(hash_utxo_key(&self.env, utxo65))
    }

    #[inline(always)]
    fn drawer_key(id: u32) -> DrawerDataKey {
        DrawerDataKey::Drawer(DrawerKey { id })
    }

    #[inline(always)]
    fn get_state(&mut self) -> DrawerState {
        if self.cache.state.is_none() {
            self.cache.state = Some(
                self.env
                    .storage()
                    .persistent()
                    .get::<_, DrawerState>(&DrawerDataKey::State)
                    .unwrap_or(DrawerState {
                        current_drawer: 1,
                        next_slot: 0,
                    }),
            );
        }

        self.cache.state.clone().unwrap()
    }

    #[inline(always)]
    fn alloc_slot_and_rotate_if_needed(&mut self) -> (u32, u32) {
        let mut state = self.get_state();

        if state.next_slot >= Self::SLOTS_PER_DRAWER {
            let old_drawer_id = state.current_drawer;
            let found_dirty = self.cache.dirty_drawers.get(old_drawer_id).unwrap_or(false);

            if found_dirty {
                if let Some(bitmap) = self.cache.drawers.get(old_drawer_id) {
                    self.env
                        .storage()
                        .persistent()
                        .set(&Self::drawer_key(old_drawer_id), &bitmap);
                    self.cache.drawers.remove(old_drawer_id);
                    self.cache.dirty_drawers.remove(old_drawer_id);
                }
            }

            state.current_drawer = state
                .current_drawer
                .checked_add(1)
                .expect("drawer overflow");
            state.next_slot = 0;
        }

        let drawer_id = state.current_drawer;
        let slot_idx = state.next_slot;
        state.next_slot += 1;

        self.cache.state = Some(state);
        self.cache.state_dirty = true;

        (drawer_id, slot_idx)
    }

    #[inline(always)]
    fn get_or_create_bitmap(&mut self, drawer_id: u32) -> Bytes {
        if let Some(bitmap) = self.cache.drawers.get(drawer_id) {
            return bitmap;
        }

        let bitmap = self
            .env
            .storage()
            .persistent()
            .get::<_, Bytes>(&Self::drawer_key(drawer_id))
            .unwrap_or_else(|| Bytes::new(&self.env));

        self.cache.drawers.set(drawer_id, bitmap.clone());
        bitmap
    }

    #[inline(always)]
    fn bitmap_byte_index(&self, slot_idx: u32) -> u32 {
        let byte_i = slot_idx >> 3;

        if slot_idx >= Self::SLOTS_PER_DRAWER || byte_i >= Self::BITMAP_BYTES {
            panic_with_error!(&self.env, Error::InvalidDrawerSlot);
        }

        byte_i
    }

    #[inline(always)]
    fn is_bit_set(&self, bitmap: &Bytes, slot_idx: u32) -> bool {
        let byte_i = self.bitmap_byte_index(slot_idx);
        let bit_mask = 1u8 << (slot_idx & 7);
        let byte = bitmap.get(byte_i).unwrap_or(0);
        (byte & bit_mask) != 0
    }

    #[inline(always)]
    fn set_bit_in_bitmap(&self, bitmap: &mut Bytes, slot_idx: u32, val: bool) -> bool {
        let byte_i = self.bitmap_byte_index(slot_idx);
        while bitmap.len() <= byte_i {
            bitmap.push_back(0u8);
        }

        let bit_mask = 1u8 << (slot_idx & 7);
        let old = bitmap.get(byte_i).unwrap_or(0);
        let new = if val { old | bit_mask } else { old & !bit_mask };

        if old != new {
            bitmap.set(byte_i, new);
            return true;
        }

        false
    }
}

fn hash_utxo_key(e: &Env, utxo65: &BytesN<65>) -> BytesN<32> {
    let b = Bytes::from_slice(e, utxo65.to_array().as_ref());
    let h = e.crypto().sha256(&b);
    BytesN::<32>::from_array(e, &h.to_array())
}
