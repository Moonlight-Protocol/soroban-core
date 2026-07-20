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

Callers do not construct storage directly. `Store::apply` creates the scoped
storage handle and lets the caller perform one logical group of operations. Each
operation writes its own per-UTXO entry directly — there is no deferred commit
step.

## Storage Model

Each UTXO owns a single persistent entry, keyed by the hash of its 65-byte
public key, whose value is the UTXO's amount:

```text
UTXOCoreDataKey::UTXO(sha256(pubkey65)) -> i128 amount
```

The amount value carries the full spend state:

- a **positive** value means the UTXO is **unspent** (and is its amount);
- `0` means the UTXO has been **spent** (a tombstone that is kept, not deleted);
- an **absent** entry means **no record exists** for that key.

Each UTXO has its own entry; no state is shared between entries.

## Operation Semantics

### `balance`

`balance(utxo)` reads the UTXO's persistent entry and returns:

- a positive amount when the entry exists and is unspent;
- `0` when the entry exists but has been spent;
- `-1` when no entry exists for the UTXO key.

Reading an existing entry refreshes its TTL (see [TTL / keep-alive](#ttl--keep-alive)),
so a holder keeps their own UTXO alive simply by observing it.

### `create`

`create(utxo, amount)` creates a new unspent UTXO.

The operation:

1. rejects any key that is not an SEC1 *uncompressed* P-256 point encoding —
   i.e. whose leading marker byte is not `0x04` (`InvalidUtxoKey`), see
   [Key validation](#key-validation-audit-a2) below;
2. rejects non-positive amounts (`InvalidCreateAmount`);
3. rejects any key that already has a record — including a spent (`0`) tombstone,
   which can never be recreated (`UtxoAlreadyExists`);
4. writes the per-UTXO entry with `amount`;
5. refreshes the entry's TTL.

#### Key validation (audit A2)

The UTXO key IS the spending credential: `__check_auth` releases a UTXO only on a
P-256 signature verifying under its key, and the host `secp256r1_verify` traps on
a malformed key. A UTXO funded under a malformed key therefore can never be spent
and its value is **locked permanently**. `create` guards against the obvious
malformations once, at creation, by requiring the 65-byte key to carry the SEC1
uncompressed marker byte `0x04` — at no per-spend cost.

This is the marker-byte *minimum*, not full on-curve validation: soroban-sdk
25.3.0 exposes no P-256 point-validation host primitive (only `secp256r1_verify`,
which needs a signature), and full on-curve checking would require in-contract
256-bit field arithmetic paid on every create. A key with a `0x04` marker but
off-curve coordinates still passes this guard and would lock on spend, so
**wallets MUST validate that a key is a valid on-curve P-256 point before funding
it** — such possession/encoding errors are undetectable on-chain regardless.

### `spend`

`spend(utxo)` marks an existing unspent UTXO as spent and returns its amount.

The operation:

1. reads the per-UTXO entry;
2. rejects a missing UTXO (`UtxoDoesNotExist`) or an already-spent one
   (`UtxoAlreadySpent`);
3. tombstones the entry in place by setting its amount to `0` — the entry is
   **not** deleted, so the spent record keeps blocking re-spend and re-creation;
4. refreshes the entry's TTL.

## TTL / keep-alive

A UTXO's entry backs user funds and must outlive long idle periods; without an
explicit bump it would archive. Each of `create`, `spend`, and `balance` extends
the touched entry's TTL (`PERSISTENT_BUMP_AMOUNT = 30 days`, refreshed when
within `PERSISTENT_LIFETIME_THRESHOLD`), including the spent tombstone.

If the closure passed to `Store::apply` panics, the invocation aborts before any
further writes; Soroban does not commit partial state changes.

## Public API

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
  operations against a scoped handle.
- `store.balance(utxo)`: reads the current UTXO balance state (`-1` / `0` / `>0`).
- `store.create(utxo, amount)`: creates a new unspent UTXO.
- `store.spend(utxo)`: spends an existing unspent UTXO and returns its amount.

There is intentionally no public cache type, no manual commit API, and no
alternate storage backend. The storage module owns its layout internally.
