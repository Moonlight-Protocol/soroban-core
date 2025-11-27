/* =========================
Drawer optimized storage with injected cache
Layout:
- Per UTXO entry stores amount, drawer_id, slot_idx (never deleted)
- Per drawer entry stores bitmap Bytes of fixed size
- Cache passed from contract layer accumulates changes
========================= */

use soroban_sdk::{contracttype, panic_with_error, Bytes, BytesN, Env, Map};

use crate::{DrawerKey, Error, UTXOCoreDataKey, UtxoStore};

#[derive(Clone)]
#[contracttype]
pub struct UtxoMeta {
    pub amount: i128,
    pub drawer_id: u32,
    pub slot_idx: u32,
}

#[derive(Clone)]
#[contracttype]
pub struct DrawerState {
    pub current_drawer: u32,
    pub next_slot: u32,
}

#[derive(Clone)]
#[contracttype]
pub enum DrawerDataKey {
    Drawer(DrawerKey), // value: Bytes bitmap
    State,             // value: DrawerState
}

// The cache that gets passed through all operations
pub struct DrawerCache {
    pub drawers: Map<u32, Bytes>,      // drawer_id -> bitmap
    pub state: Option<DrawerState>,    // Cached drawer state
    pub state_dirty: bool,             // Track if state needs writing
    pub dirty_drawers: Map<u32, bool>, // Track which drawers need writing
}

impl DrawerCache {
    pub fn new(e: &Env) -> Self {
        DrawerCache {
            drawers: Map::new(e),
            state: None,
            state_dirty: false,
            dirty_drawers: Map::new(e),
        }
    }

    // Commit all cached changes to storage
    pub fn commit(&self, e: &Env) {
        // Write state if it was modified
        if self.state_dirty {
            if let Some(ref state) = self.state {
                e.storage().persistent().set(&DrawerDataKey::State, state);
            }
        }

        // Write all dirty drawers
        for (drawer_id, _) in self.dirty_drawers.iter() {
            if let Some(bitmap) = self.drawers.get(drawer_id) {
                e.storage()
                    .persistent()
                    .set(&DrawerStore::drawer_key(drawer_id), &bitmap);
            }
        }
    }
}

pub struct DrawerStore;

impl DrawerStore {
    const SLOTS_PER_DRAWER: u32 = 1024; // 128 bytes - medium-large

    const BITMAP_BYTES: u32 = Self::SLOTS_PER_DRAWER / 8;

    #[inline(always)]
    fn utxo_key(e: &Env, utxo65: &BytesN<65>) -> UTXOCoreDataKey {
        UTXOCoreDataKey::UTXO(<DrawerStore as UtxoStore>::hash_utxo_key(e, utxo65))
    }

    #[inline(always)]
    pub fn drawer_key(id: u32) -> DrawerDataKey {
        DrawerDataKey::Drawer(DrawerKey { id })
    }

    // Get state from cache or load from storage (no write back yet)
    #[inline(always)]
    fn get_state_cached(e: &Env, cache: &mut DrawerCache) -> DrawerState {
        if cache.state.is_none() {
            cache.state = Some(
                e.storage()
                    .persistent()
                    .get::<_, DrawerState>(&DrawerDataKey::State)
                    .unwrap_or(DrawerState {
                        current_drawer: 1,
                        next_slot: 0,
                    }),
            );
        }
        cache.state.clone().unwrap()
    }

    // Allocate slot using cached state
    #[inline(always)]
    fn alloc_slot_and_rotate_if_needed_cached(e: &Env, cache: &mut DrawerCache) -> (u32, u32) {
        let mut state = Self::get_state_cached(e, cache);

        // Check rotation
        if state.next_slot >= Self::SLOTS_PER_DRAWER {
            // When rotating, we might want to flush the previous drawer's bitmap
            // to free up cache space, but only if it's dirty

            // Find if the current drawer is in dirty_drawers
            let mut found_dirty = false;
            for i in 0..cache.dirty_drawers.keys().len() {
                if let Some(key) = cache.dirty_drawers.keys().get(i) {
                    if key == state.current_drawer {
                        found_dirty = true;
                        break;
                    }
                }
            }

            if found_dirty {
                let old_drawer_id = state.current_drawer;
                if let Some(bitmap) = cache.drawers.get(old_drawer_id) {
                    e.storage()
                        .persistent()
                        .set(&Self::drawer_key(old_drawer_id), &bitmap);
                    cache.drawers.remove(old_drawer_id);
                    cache.dirty_drawers.remove(old_drawer_id);
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

        // Update cache instead of storage
        cache.state = Some(state);
        cache.state_dirty = true; // Mark state as needing write

        (drawer_id, slot_idx)
    }

    // Get bitmap from cache or load from storage
    #[inline(always)]
    fn get_or_create_bitmap_cached(e: &Env, cache: &mut DrawerCache, drawer_id: u32) -> Bytes {
        // Check cache first
        if let Some(bitmap) = cache.drawers.get(drawer_id) {
            return bitmap;
        }

        // Load from storage and cache it
        let bitmap = e
            .storage()
            .persistent()
            .get::<_, Bytes>(&Self::drawer_key(drawer_id))
            .unwrap_or_else(|| {
                let mut b = Bytes::new(e);
                for _ in 0..Self::BITMAP_BYTES {
                    b.push_back(0u8);
                }
                b
            });

        cache.drawers.set(drawer_id, bitmap.clone());
        bitmap
    }

    #[inline(always)]
    fn is_bit_set(bitmap: &Bytes, slot_idx: u32) -> bool {
        let byte_i = slot_idx >> 3;
        let bit_mask = 1u8 << (slot_idx & 7);
        let byte = bitmap.get(byte_i).unwrap_or(0);
        (byte & bit_mask) != 0
    }

    #[inline(always)]
    fn set_bit_in_bitmap(bitmap: &mut Bytes, slot_idx: u32, val: bool) -> bool {
        let byte_i = slot_idx >> 3;
        let bit_mask = 1u8 << (slot_idx & 7);
        let old = bitmap.get(byte_i).unwrap_or(0);
        let new = if val { old | bit_mask } else { old & !bit_mask };

        if old != new {
            bitmap.set(byte_i, new);
            return true;
        }
        false
    }

    // New methods that accept cache
    pub fn utxo_balance_cached(e: &Env, cache: &mut DrawerCache, utxo65: &BytesN<65>) -> i128 {
        let k = Self::utxo_key(e, &utxo65);
        match e.storage().persistent().get::<_, UtxoMeta>(&k) {
            Some(UtxoMeta {
                amount,
                drawer_id,
                slot_idx,
            }) => {
                let bitmap = Self::get_or_create_bitmap_cached(e, cache, drawer_id);
                if Self::is_bit_set(&bitmap, slot_idx) {
                    amount
                } else {
                    0
                }
            }
            None => -1,
        }
    }

    pub fn create_cached(e: &Env, cache: &mut DrawerCache, utxo65: &BytesN<65>, amount: i128) {
        if amount <= 0 {
            panic_with_error!(e, Error::InvalidCreateAmount);
        }

        let uk = Self::utxo_key(e, &utxo65);

        if let Some(UtxoMeta {
            drawer_id,
            slot_idx,
            ..
        }) = e.storage().persistent().get::<_, UtxoMeta>(&uk)
        {
            let bitmap = Self::get_or_create_bitmap_cached(e, cache, drawer_id);
            if Self::is_bit_set(&bitmap, slot_idx) {
                panic_with_error!(e, Error::UTXOAlreadyExists);
            }
        }

        // Use cached allocation
        let (drawer_id, slot_idx) = Self::alloc_slot_and_rotate_if_needed_cached(e, cache);

        // Store UTXO (this still goes directly to storage)
        e.storage().persistent().set(
            &uk,
            &UtxoMeta {
                amount,
                drawer_id,
                slot_idx,
            },
        );

        // Update bitmap in cache
        let mut bitmap = Self::get_or_create_bitmap_cached(e, cache, drawer_id);
        Self::set_bit_in_bitmap(&mut bitmap, slot_idx, true);
        cache.drawers.set(drawer_id, bitmap);
        cache.dirty_drawers.set(drawer_id, true);
    }

    pub fn spend_cached(e: &Env, cache: &mut DrawerCache, utxo65: &BytesN<65>) -> i128 {
        let uk = Self::utxo_key(e, &utxo65);
        match e.storage().persistent().get::<_, UtxoMeta>(&uk) {
            Some(UtxoMeta {
                amount,
                drawer_id,
                slot_idx,
            }) => {
                // Get bitmap from cache
                let mut bitmap = Self::get_or_create_bitmap_cached(e, cache, drawer_id);

                if !Self::is_bit_set(&bitmap, slot_idx) {
                    panic_with_error!(e, Error::UTXOAlreadySpent);
                }

                // Update bitmap in cache only
                Self::set_bit_in_bitmap(&mut bitmap, slot_idx, false);
                cache.drawers.set(drawer_id, bitmap);
                cache.dirty_drawers.set(drawer_id, true);

                amount
            }
            None => panic_with_error!(e, Error::UTXONotFound),
        }
    }
}

// Keep the old trait impl for backward compatibility
impl UtxoStore for DrawerStore {
    fn utxo_balance(e: &Env, utxo65: &BytesN<65>) -> i128 {
        let mut cache = DrawerCache::new(e);
        Self::utxo_balance_cached(e, &mut cache, &utxo65)
    }

    fn create(e: &Env, utxo65: &BytesN<65>, amount: i128) {
        let mut cache = DrawerCache::new(e);
        Self::create_cached(e, &mut cache, &utxo65, amount);
        cache.commit(e);
    }

    fn spend(e: &Env, utxo65: &BytesN<65>) -> i128 {
        let mut cache = DrawerCache::new(e);
        let result = Self::spend_cached(e, &mut cache, &utxo65);
        cache.commit(e);
        result
    }
}
