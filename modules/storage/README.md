# Moonlight Storage

The storage module owns the persistent UTXO storage layout used by
`moonlight-utxo-core`.

It exposes a single scoped API:

```rust
Store::apply(env, |store| {
    let amount = store.balance(&utxo);
    store.spend(&utxo);
    store.create(&next_utxo, amount);
});
```

Callers do not construct storage directly and do not commit changes manually.
`Store::apply` creates the scoped storage handle, lets the caller perform one
logical group of operations, and commits dirty drawer data after the closure
returns.

## Why This Storage Exists

A straightforward UTXO storage model would store the full state for each UTXO:

```text
UTXO(hash(pubkey)) -> Unspent(amount) | Spent
```

That model is simple, but it scales writes with the number of UTXOs spent. If a
transaction spends 20 UTXOs, it needs to update 20 independent storage entries
just to mark them as spent.

Moonlight expects many UTXOs to be created and spent in clustered flows. UTXOs
created close together are likely to be spent together because the protocol is
normally used in ordered batches. The drawer layout exploits that locality.
Instead of storing the mutable spent/unspent flag in every UTXO entry, it stores
that flag as one bit inside a shared drawer bitmap.

This means a transaction that spends several UTXOs from the same drawer can
update several bits and write one drawer entry back. The main benefit is
reducing the number of mutable storage entries written during clustered spends,
which matters because network limits and fees are sensitive to storage entries
touched and written.

## Storage Layout

Each UTXO has one metadata entry:

```text
UTXOCoreDataKey::UTXO(sha256(pubkey65)) -> UtxoMeta
```

The metadata stores:

```rust
UtxoMeta {
    amount: i128,
    drawer_id: u32,
    slot_idx: u32,
}
```

The metadata tells the storage module:

- the UTXO amount;
- which drawer bitmap contains its spent/unspent flag;
- which bit inside that drawer represents this UTXO.

The drawer bitmap stores only status:

```text
DrawerDataKey::Drawer(DrawerKey { id }) -> Bytes bitmap
```

Inside the bitmap:

- bit set to `1` means the UTXO is unspent;
- bit set to `0` means the UTXO is spent or the slot has not been allocated.

There is also one drawer allocator state entry:

```text
DrawerDataKey::State -> DrawerState {
    current_drawer,
    next_slot,
}
```

`current_drawer` identifies where new UTXOs are currently allocated.
`next_slot` is the next available slot in that drawer.

## Drawer Size

Each drawer has 524,288 slots:

```rust
const SLOTS_PER_DRAWER: u32 = 524_288;
const BITMAP_BYTES: u32 = SLOTS_PER_DRAWER / 8; // 65,536 bytes
```

One bit represents one UTXO status. One byte therefore represents 8 UTXOs.

The maximum bitmap payload is 65,536 bytes, which is intentionally below the
128 KiB ledger-entry size limit. The bitmap is not allocated at full size when a
drawer is created. It grows lazily as higher slot indexes are used, avoiding the
cost of pushing 65,536 zero bytes when the first UTXO is created.

## Operation Semantics

### `balance`

`balance(utxo)` reads the UTXO metadata and then reads the corresponding bit in
the drawer bitmap.

It returns:

- a positive amount when the metadata exists and the drawer bit is set;
- `0` when the metadata exists but the drawer bit is clear;
- `-1` when no metadata entry exists for the UTXO key.

### `create`

`create(utxo, amount)` creates a new unspent UTXO.

The operation:

1. rejects non-positive amounts;
2. rejects UTXO keys that already have metadata;
3. allocates the next drawer slot;
4. writes the UTXO metadata with `amount`, `drawer_id`, and `slot_idx`;
5. sets the corresponding drawer bitmap bit to `1`;
6. marks the drawer bitmap and drawer state as dirty in the scoped cache.

The UTXO metadata is stable after creation. The mutable status lives in the
drawer bitmap.

### `spend`

`spend(utxo)` marks an existing unspent UTXO as spent and returns its amount.

The operation:

1. reads the UTXO metadata;
2. loads the drawer bitmap identified by `drawer_id`;
3. checks the bit at `slot_idx`;
4. rejects missing or already-spent UTXOs;
5. clears the bitmap bit to `0`;
6. marks the drawer bitmap as dirty in the scoped cache.

The metadata entry is not deleted. Keeping metadata lets the module distinguish
between a spent UTXO (`0`) and a never-created UTXO (`-1`).

## Scoped Cache

The drawer cache is private to the storage module.

During `Store::apply`, the cache:

- loads `DrawerState` only when allocation is needed;
- loads drawer bitmaps on demand;
- keeps modified drawer bitmaps in memory;
- tracks which drawer bitmaps are dirty;
- writes dirty drawer bitmaps once when the scope completes;
- writes drawer allocation state once when it changed.

This is what lets a batch update many bits in the same drawer while writing the
drawer entry once at the end of the scope.

If the closure passed to `Store::apply` panics, the commit step is not reached.
That is expected: Soroban aborts the invocation, so partial state changes are
not committed.

## Public API

Use the scoped API for every storage interaction:

```rust
Store::apply(env, |store| {
    let balance = store.balance(&utxo);

    if balance > 0 {
        store.spend(&utxo);
    }
});
```

The public operations are:

- `Store::apply(env, |store| { ... })`: runs one logical group of storage
  operations with a private drawer cache.
- `store.balance(utxo)`: reads the current UTXO balance state.
- `store.create(utxo, amount)`: creates a new unspent UTXO.
- `store.spend(utxo)`: spends an existing unspent UTXO and returns its amount.

There is intentionally no public cache type, no manual commit API, and no
alternate storage backend. The storage module owns the drawer layout and cache
lifecycle internally.
