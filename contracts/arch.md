# Soroban Core — Contract Architecture

This document describes the in-scope smart contracts in `Moonlight-Protocol/soroban-core` for security audit purposes. It targets readers who have not seen the codebase before and need to build a working model of the protocol from the contract perspective.

The audit scope is exactly the two contracts in `contracts/`:

- `contracts/channel-auth/` — the **Channel Auth** contract (referred to as *Quorum Auth* in the README).
- `contracts/privacy-channel/` — the **Privacy Channel** contract.

The supporting modules in `modules/` (`utxo-core`, `auth`, `primitives`, `storage`, `helpers`) are linked in as `rlib` workspace dependencies and ship as part of the contract WASMs. They are in scope insofar as the contracts depend on them, but they do not deploy as standalone contracts.

The `contracts/token/` directory is a test-only token used by `privacy-channel` integration tests; it is **not** in audit scope and is not deployed.

Off-chain components — provider platform, browser wallet, council-console, network-dashboard, the moonlight-sdk, and the local-dev orchestration — are out of scope.

---

## 1. Protocol overview from the contract perspective

Moonlight is a privacy-preserving asset channel built on Soroban. At the contract layer the protocol consists of two compositional pieces:

1. **Privacy Channel** holds a single Stellar asset (an SAC — Stellar Asset Contract — address) and tracks its supply across an internal UTXO set. Users move asset balances into the channel via `ExtDeposit`, transfer privately within the channel via `Spend`/`Create`, and exit via `ExtWithdraw`. All of this happens through a single unified entry point: `transact(op: ChannelOperation)`.

2. **Channel Auth** is a separate contract that implements Soroban's `CustomAccountInterface`. The Privacy Channel stores a Channel Auth contract address as its authorization principal. Whenever the Privacy Channel needs to authorize an internal UTXO operation, it calls `require_auth_for_args` on this Channel Auth address, which in turn invokes `__check_auth` on the Channel Auth contract. Channel Auth verifies (a) at least one registered Privacy Provider has signed the bundle, and (b) every spending UTXO owner has signed the conditions they intend to authorize.

A single Channel Auth contract may govern multiple Privacy Channel contracts (the README refers to this as a "quorum"). The Channel Auth has its own admin (typically a Stellar account with native multisig representing the Moonlight Security Council) who can add/remove providers, transfer admin rights, and upgrade either contract.

### 1.1 Composition at runtime

A typical end-to-end flow when a user transfers privately:

1. User assembles a `ChannelOperation` off-chain (with the help of moonlight-sdk): a list of UTXOs to spend, UTXOs to create, deposits and withdrawals.
2. Each UTXO owner produces a P256 (secp256r1) ECDSA signature over a hash of the conditions they authorize, plus a `live_until_ledger`.
3. A registered Privacy Provider produces a Stellar-native auth-entry signature (Ed25519) over the canonical Soroban authorization payload for the `transact` invocation.
4. The provider submits the transaction. The Privacy Channel's `transact` function:
   - validates external-operation structure (no duplicate addresses; no condition conflicts);
   - delegates UTXO-set mutation to `process_bundle` (from `moonlight-utxo-core::core::UtxoHandlerTrait`), which calls `auth.require_auth_for_args(...)` against Channel Auth;
   - Soroban host invokes `Channel Auth::__check_auth`, which verifies the provider Ed25519 signature and every per-UTXO P256 signature;
   - `process_bundle` applies spends and creates atomically, asserting balance equality;
   - `transact` then dispatches `ExtDeposit` and `ExtWithdraw` operations through the asset SAC, adjusting `Supply` accordingly.

Any failure at any step reverts the entire transaction.

---

## 2. Channel Auth (`contracts/channel-auth/`)

### 2.1 Responsibilities

- Maintain the registered set of Privacy Providers authorized to sign bundles.
- Maintain the admin address with rights to mutate the provider set, transfer admin, and upgrade contracts.
- Implement `CustomAccountInterface` so that other contracts (specifically, Privacy Channel instances) can use a Channel Auth contract address as their authorization principal.
- Verify provider Ed25519 signatures and per-UTXO P256 signatures on every bundle.

### 2.2 Public interface

Source of truth: `contracts/channel-auth/src/contract.rs`.

| Function | Caller | Args | Returns | Purpose |
|---|---|---|---|---|
| `__constructor(admin)` | Soroban runtime (deploy time) | `admin: Address` | — | Sets admin, emits `ContractInitialized`. |
| `add_provider(provider)` | admin | `provider: Address` | — | Registers a provider. Emits `ProviderAdded`. Panics if already registered. |
| `remove_provider(provider)` | admin | `provider: Address` | — | Deregisters a provider. Emits `ProviderRemoved`. Panics if not registered. |
| `is_provider(provider)` | anyone | `provider: Address` | `bool` | Read-only membership query. |
| `set_admin(new_admin)` | admin | `new_admin: Address` | — | From `admin-sep::Administratable`. Transfers admin role. |
| `admin()` | anyone | — | `Address` | From `admin-sep::Administratable`. Read current admin. |
| `upgrade(wasm_hash)` | admin | `wasm_hash: BytesN<32>` | — | From `admin-sep::Upgradable`. Replaces contract WASM. |
| `__check_auth(payload, signatures, contexts)` | Soroban host | `payload: Hash<32>`, `signatures: Signatures`, `contexts: Vec<Context>` | `Result<(), Error>` | Auth entry point invoked by Soroban when this contract is named as an authorization principal. |

The `Administratable` and `Upgradable` traits are pulled from the theahaco fork of `admin-sep` (workspace dep, rev `bf195f4`). They provide standard admin-gated patterns and require admin auth at the entry point.

### 2.3 Persistent state (instance storage)

Keys (all under `e.storage().instance()`):

- `Admin` — `Address`. Set in constructor; mutated by `set_admin`. (Encoded by admin-sep, not by this contract directly.)
- `ProviderDataKey::AuthorizedProvider(addr)` — `()`. One entry per registered provider. Membership is checked via `.get(...).is_some()`.

Storage is **instance** (lives with the contract, has the contract's TTL) — not persistent. This means provider set lookups are cheap (single instance read) but the provider set must fit in a single instance entry's encoded size.

There is no nonce, no per-account replay state, and no rate-limiting state in Channel Auth. Replay protection comes entirely from Soroban's authorization-entry nonce (managed by the host) and from per-signature `live_until_ledger` expiry checks.

### 2.4 Events emitted

- `contract_initialized` — `{ admin: Address }`. Topic-formatted via `#[contractevent]`.
- `provider_added` — `{ provider: Address }`.
- `provider_removed` — `{ provider: Address }`.

There is **no event emitted on `set_admin` or `upgrade`** by this contract directly. Whatever auditing trail exists for those actions comes from the underlying `admin-sep` traits and the Soroban transaction record.

### 2.5 Auth flow detail

On every `transact` against a Privacy Channel governed by a Channel Auth instance, Soroban invokes:

```
Channel Auth::__check_auth(payload, signatures, contexts)
```

with:

- `payload` = the canonical Soroban authorization-entry hash for the `transact(...)` invocation (network ID, nonce, expiration ledger, root invocation).
- `signatures: Signatures` = `Map<SignerKey, (Signature, valid_until_ledger: u32)>`. Provided by the transaction submitter.
- `contexts: Vec<Context>` = the tree of `require_auth_for_args` sites along this auth chain.

The implementation runs two checks in sequence (`contracts/channel-auth/src/contract.rs:78-87`):

**(a) `require_provider(payload, signatures)`** — defined in `moonlight-auth::core::ProviderAuthorizable`:

- Iterates `signatures.0.keys()`.
- For each `SignerKey::Provider(pk32)`: convert pk32 → G… address; assert it is a registered provider; assert the signature has not expired (`valid_until_ledger >= current_ledger_sequence`); verify the Ed25519 signature against `payload`.
- Counts valid provider signatures into `provider_quorum`.
- Errors with `ProviderThresholdNotMet` unless `provider_quorum >= PROVIDER_THRESHOLD`. **`PROVIDER_THRESHOLD` is hardcoded to 1.**

**(b) `handle_utxo_auth(signatures, contexts)`** — defined in `moonlight-auth::core::UtxoAuthorizable`:

- For each context in `contexts`:
  - Reject if not a `Context::Contract` (e.g. `Context::CreateContractHostFn`) — error `UnexpectedContext`.
  - If the contract context has zero `args`, return `Ok(())` without further checks (interpreted as "no auth requirements for this call site").
  - Otherwise, parse `args[0]` as `AuthRequirements` (`Map<SignerKey, Vec<Condition>>`).
  - For each entry in the map whose key is `SignerKey::P256(...)`:
    - Look up the corresponding `(Signature, valid_until_ledger)` in `signatures`.
    - Reject expired signatures (`SignatureExpired`).
    - Recompute the per-UTXO auth payload as `hash_payload(AuthPayload { conditions, live_until_ledger }, caller_contract_address_bytes)` (see `moonlight-primitives::hash_payload`).
    - Verify the secp256r1 signature against this hash.
  - Map entries whose key is not `SignerKey::P256(...)` are silently skipped (with `continue`). In production this never matters because `calculate_auth_requirements` only constructs P256 entries.

Both checks must succeed for `__check_auth` to return `Ok(())`. The Soroban host treats anything else as failed auth and aborts the transaction.

### 2.6 Trust assumptions for Channel Auth

| Principal | Trust | Capabilities |
|---|---|---|
| Admin | Highest | Add/remove providers; set new admin; upgrade contract WASM. Compromise of the admin key compromises the entire auth surface and any Privacy Channels governed by this Channel Auth. |
| Provider | Bundle-level | Can authorize any well-formed bundle by producing a valid Ed25519 signature over its auth payload. Cannot mint, burn, or move UTXOs they do not co-authorize via P256 signatures. |
| UTXO owner (P256) | Per-UTXO | Can authorize spending of UTXOs whose public key matches their P256 secret. Cannot bypass the provider check. |
| Soroban host | Implicit | Signature verification primitives (`secp256r1_verify`, `ed25519_verify`) are trusted to panic on invalid signatures. The verify wrappers in `modules/auth/src/core.rs` rely on this panic-on-failure semantic and do not propagate a failure result of their own. |

---

## 3. Privacy Channel (`contracts/privacy-channel/`)

### 3.1 Responsibilities

- Hold a balance of a single Stellar asset (the asset address is set at construction time and never overwritten in any code path).
- Maintain a UTXO set keyed by 65-byte SEC1-uncompressed P256 public keys, persisted via the storage backend selected at compile time.
- Track the channel's total `Supply` — the sum of all unspent UTXO amounts that originated from `ExtDeposit` minus all `ExtWithdraw` amounts.
- Expose a single mutating entry point — `transact(op: ChannelOperation)` — that atomically processes any combination of spends, creates, deposits, and withdrawals.
- Delegate authorization to its configured Channel Auth contract.

### 3.2 Public interface

Source of truth: `contracts/privacy-channel/src/contract.rs`.

| Function | Caller | Args | Returns | Purpose |
|---|---|---|---|---|
| `__constructor(admin, auth_contract, asset)` | Soroban runtime | `admin: Address`, `auth_contract: Address`, `asset: Address` | — | Sets admin, requires admin auth, sets the auth contract address, writes the asset address. |
| `asset()` | anyone | — | `Address` | Returns the asset SAC address. |
| `supply()` | anyone | — | `i128` | Returns current channel supply. |
| `transact(op)` | anyone (with valid auth) | `op: ChannelOperation` | — | Unified bundle-processing entry point. |
| `auth()` | anyone | — | `Address` | From `UtxoHandlerTrait`. Returns Channel Auth contract address. |
| `set_auth(new_auth)` | host invocations only | `new_auth: Address` | — | From `UtxoHandlerTrait`. Marked `#[internal]`; not directly invokable by external callers. |
| `utxo_balance(utxo)` | anyone | `utxo: BytesN<65>` | `i128` | Reads UTXO state. Returns positive amount if unspent, `0` if spent, `-1` if no record exists. |
| `utxo_balances(utxos)` | anyone | `utxos: Vec<BytesN<65>>` | `Vec<i128>` | Batch wrapper around `utxo_balance`. |
| `set_admin(new_admin)` | admin | `new_admin: Address` | — | From `admin-sep::Administratable`. |
| `admin()` | anyone | — | `Address` | From `admin-sep::Administratable`. |
| `upgrade(wasm_hash)` | admin | `wasm_hash: BytesN<32>` | — | From `admin-sep::Upgradable`. |

`ChannelOperation` is defined in `contracts/privacy-channel/src/transact.rs`:

```rust
pub struct ChannelOperation {
    pub spend:    Vec<(BytesN<65>, Vec<Condition>)>,
    pub create:   Vec<(BytesN<65>, i128)>,
    pub deposit:  Vec<(Address, i128, Vec<Condition>)>,
    pub withdraw: Vec<(Address, i128, Vec<Condition>)>,
}
```

`Condition` is defined in `modules/primitives/src/lib.rs`:

```rust
pub enum Condition {
    Create(BytesN<65>, i128),                       // expected outgoing UTXO
    ExtDeposit(Address, i128),                      // expected deposit from address
    ExtWithdraw(Address, i128),                     // expected withdrawal to address
    ExtIntegration(Address, Vec<BytesN<65>>, i128), // adapter address, keys, amount
}
```

The four operation types in the README — Create, Spend, ExtDeposit, ExtWithdraw — correspond exactly to the four fields above. `ExtIntegration` is a condition variant (not an operation type) used to express adapter-mediated cross-flows, but the contracts themselves do not currently consume this variant during execution; `execute_external_operations` only iterates `deposit` and `withdraw`.

### 3.3 Persistent state

**Instance storage** (lifetime tied to contract):

- `PrivacyChannelDataKey::Asset` — `Address`. Written exactly once in `__constructor` via `write_asset_unchecked` and never touched again. There is no `set_asset` function.
- `PrivacyChannelDataKey::Supply` — `i128`. Mutated by `increase_supply` / `decrease_supply` (in `treasury.rs`) on `ExtDeposit` / `ExtWithdraw`.
- `STORAGE_KEY_UTXO_AUTH` (symbol `"UTXO_AUTH"`) — `Address`. Written in `__constructor` via `set_auth`. There is no exposed external mutator.
- `Admin` — `Address`, encoded by admin-sep.

**Persistent storage** (per-UTXO, lives independently of contract instance TTL):

The active backend is selected at compile time via mutually exclusive cargo features `storage-simple` and `storage-drawer`. The default selected by `moonlight-utxo-core` is `storage-drawer`.

- **`storage-simple`** (`modules/storage/src/simple.rs`): each UTXO is a separate persistent entry keyed by `UTXOCoreDataKey::UTXO(sha256(pk65))`, value `UtxoState::Unspent(i128) | Spent`.
- **`storage-drawer`** (`modules/storage/src/drawer.rs`): each UTXO is a `UtxoMeta { amount, drawer_id, slot_idx }` entry plus a bit in a 1024-slot bitmap stored at `DrawerDataKey::Drawer(DrawerKey { id })`. There is also a `DrawerDataKey::State` entry that tracks the current allocation pointer (`current_drawer: u32`, `next_slot: u32`). The drawer backend is intended to amortize storage cost across many UTXOs by packing the spent/unspent flag into a shared bitmap.

UTXO keys are hashed (sha256) before being stored, so storage uses 32-byte keys instead of 65-byte ones. This is a cost optimization; collision resistance comes from sha256.

The Privacy Channel contract is configured to consume `moonlight-utxo-core` with the `no-utxo-events` and `no-bundle-events` cargo features enabled (see `contracts/privacy-channel/Cargo.toml`). This **suppresses** the per-UTXO and bundle-level event emissions that `moonlight-utxo-core` would otherwise publish. **The Privacy Channel itself does not emit any events from `transact`.** The only on-chain event trail for a transact invocation comes from the underlying SAC `transfer` calls during `ExtDeposit` and `ExtWithdraw`. Bundles that consist purely of internal `spend` and `create` operations leave **no Soroban event behind** — their existence is visible only in transaction footprints, fees, and the resulting UTXO storage state.

### 3.4 Events emitted

None directly from `Privacy Channel`. Indirect events:

- SAC `transfer` events on `ExtDeposit` (asset → channel) and `ExtWithdraw` (channel → asset).
- `ContractInitialized`, `ProviderAdded`, `ProviderRemoved` from the Channel Auth contract that governs this channel (separate contract).

### 3.5 Auth flow detail

`transact` runs three phases:

**(a) `pre_process_channel_operation`** — `contracts/privacy-channel/src/transact.rs:39`:

- `op_has_no_conflicting_conditions(&e, &op)` — flatten the conditions across spend/deposit/withdraw and check pairwise `Condition::conflicts_with`. Conflicts include:
  - Two `Create(utxo, a)` and `Create(utxo, b)` with `a != b` (same target, different amount).
  - Two `ExtDeposit(addr, a)` / `ExtDeposit(addr, b)` with `a != b`. (Same for `ExtWithdraw`.)
  - Two `ExtIntegration` entries that overlap UTXOs across different adapters, or differ in amount/UTXO-set within the same adapter.
- Sum `total_deposit` and `total_withdraw` over the deposit/withdraw lists, with `checked_add` overflow detection (errors `AmountOverflow`).
- `verify_external_operations`:
  - No duplicate addresses in `deposit` or `withdraw` (errors `RepeatedAccountForDeposit` / `RepeatedAccountForWithdraw`).
  - If an address appears in *both* deposit and withdraw, the two condition sequences must be byte-equal under XDR encoding (errors `ConflictingConditionsForAccount`). This is stricter than the conflict-free check above and is the only path through which an address may legitimately appear on both sides.
- Build `AuthRequirements` from the `spend` list via `calculate_auth_requirements`: one P256 entry per (utxo, conditions) pair.
- Build `InternalBundle { spend, create, req }` and return it along with the deposit/withdraw totals.

**(b) `Self::process_bundle(env, bundle, total_deposit, total_withdraw)`** — `modules/utxo-core/src/core.rs:96-208`:

- Assert no duplicate UTXO keys in `bundle.spend` (errors `RepeatedSpendUTXO`) or `bundle.create` (errors `RepeatedCreateUTXO`).
- Construct `auth_args = vec![&e]` if `bundle.req.0.is_empty()` (no spends), else `vec![&e, bundle.req.into_val(e)]`.
- Call `Self::auth().require_auth_for_args(auth_args)`. This is the line that triggers the Soroban host to invoke `Channel Auth::__check_auth`.
- For each `spend_utxo` in `bundle.spend`: read the UTXO's current balance, panic if 0 (`UTXOAlreadySpent`) or -1 (`UTXODoesntExist`), mark spent, accumulate amount to `total_available_balance`.
- For each `(create_utxo, amount)` in `bundle.create`: assert the UTXO does not yet exist (panic `UTXOAlreadyExists` if it does), assert `amount > 0` (panic `InvalidCreateAmount`), allocate, deduct from `total_available_balance`.
- Final invariant check: `total_available_balance == expected_outgoing` (the `total_withdraw` argument). Panic `UnbalancedBundle` if not.
- Bundle and per-UTXO events would be published here, but are suppressed by the `no-bundle-events` and `no-utxo-events` features.

**(c) `execute_external_operations(deposit, withdraw)`** — `contracts/privacy-channel/src/transact.rs:112`:

- For each deposit `(from, amount, conditions)`:
  - `from.require_auth_for_args(vec![&e, conditions.into_val(&e)])` — requires the depositor to authorize this exact set of conditions.
  - `asset_client.transfer(&from, &channel, &amount)` — pulls funds in.
  - `increase_supply(&e, amount)`.
- For each withdrawal `(to, amount, _conditions)`:
  - `e.authorize_as_current_contract(...)` — the channel contract self-authorizes the outbound transfer.
  - `asset_client.transfer(&channel, &to, &amount)`.
  - `decrease_supply(&e, amount)`.

If any phase panics, the entire transaction reverts.

### 3.6 Trust assumptions for Privacy Channel

| Principal | Trust | Capabilities |
|---|---|---|
| Admin (own admin, distinct from Channel Auth admin in general) | High | Transfer admin, upgrade WASM. Compromise allows replacing contract logic on next upgrade. |
| Channel Auth contract | High | Indirect — every UTXO operation flows through this contract's `__check_auth`. Compromise of the Channel Auth's admin or providers compromises this channel. |
| Providers (registered in Channel Auth) | Bundle-level | Authorize entire bundles (threshold 1). Cannot mint UTXOs or move UTXOs whose P256 owners did not co-sign. |
| UTXO owners (P256) | Per-UTXO | Authorize spending of their own UTXOs subject to specific conditions. |
| Depositors (Stellar G-accounts) | Per-deposit | Authorize moving asset balance into the channel under specific receive-side conditions. |
| Withdraw recipients | None required | The contract self-authorizes outbound transfers; recipients do not sign. This means anyone with the right combination of signatures can name anyone as a withdrawal recipient — recipient consent is not a contract concern. |
| Asset SAC | High | Trusted to enforce its own transfer semantics. The Privacy Channel does not validate the asset contract beyond storing its address; it assumes a well-behaved SAC. |

---

## 4. Invariants

The contracts are intended to uphold each of the following invariants. Where they map to specific code locations, those are noted.

### 4.1 Channel Auth invariants

- **CA-1 (admin gating).** Provider mutations (`add_provider`, `remove_provider`), admin transfer (`set_admin`), and contract upgrade (`upgrade`) require admin auth. *Enforced by `Self::require_admin(e)` at the top of each path and the `admin-sep` traits.*
- **CA-2 (provider threshold).** No `__check_auth` succeeds unless at least one signature in `signatures` is `(SignerKey::Provider(...), Signature::Ed25519(...))` from a currently registered provider, valid against the Soroban auth-entry payload, with an unexpired `valid_until_ledger`. *Enforced by `require_provider`.*
- **CA-3 (no expired sigs).** `__check_auth` rejects any signature whose `valid_until_ledger < current_ledger_sequence`. *Enforced in both `require_provider` and `handle_utxo_auth`.*
- **CA-4 (P256 coverage).** For every P256 signer present in the per-context `AuthRequirements` map, there must be a corresponding valid P256 signature in `signatures` over `hash_payload(conditions, live_until_ledger, contract_address_bytes)`. Missing entries error `MissingSignature`. *Enforced in `handle_utxo_auth`.*
- **CA-5 (context-shape).** Only `Context::Contract` contexts are accepted. Any other context variant errors `UnexpectedContext`. *Enforced in `handle_utxo_auth`.*
- **CA-6 (signature/key match).** Each verified signer/signature pair must have matching curve types: P256 signer ↔ P256 signature, Provider/Ed25519 signer ↔ Ed25519 signature. Mismatches error `InvalidSignatureFormat`. *Enforced by `verify_signature` in `modules/auth/src/core.rs`.*
- **CA-7 (event coverage for governance).** Every change to the provider set emits the corresponding event (`ProviderAdded` / `ProviderRemoved`). *Enforced in `add_provider` / `remove_provider`.*

Known invariant gap in CA-7: `set_admin` and `upgrade` do not emit explicit events from this contract; their audit trail relies on whatever admin-sep emits and on the Stellar transaction record itself. This is documented in §6.

### 4.2 Privacy Channel invariants

- **PC-1 (immutable asset binding).** Once set in the constructor, `Asset` is never overwritten by any code path. The only writer is `write_asset_unchecked`, which is only called from `__constructor`. There is no setter exposed externally. *Enforced by code structure.*
- **PC-2 (immutable auth binding).** The auth-contract address (`STORAGE_KEY_UTXO_AUTH`) is set in the constructor and has no externally callable setter. The internal `set_auth` is `#[internal]` and is therefore not in the contract's external interface. *Enforced by code structure.*
- **PC-3 (supply ↔ external flow).** `Supply` increases only via `increase_supply` (called once per `ExtDeposit` in `execute_external_operations`) and decreases only via `decrease_supply` (called once per `ExtWithdraw`). It does not change in response to internal `Spend` / `Create`. *Enforced by `transact.rs:121-148`.*
- **PC-4 (bundle balance).** `process_bundle` enforces `total_available_balance == expected_outgoing` at the end of each bundle. Net effect: `sum_spent + total_deposit == sum_created + total_withdraw`. *Enforced by `core.rs:189-193`.*
- **PC-5 (UTXO uniqueness — create).** No UTXO key may be created if any prior record exists for it (whether unspent or spent). *Enforced by `verify_utxo_not_exists` (simple) or `is_bit_set` check + meta lookup (drawer).*
- **PC-6 (UTXO uniqueness — spend).** No UTXO may be spent twice, nor may a UTXO that has never been created be spent. *Enforced by `verify_utxo_unspent` (simple) or `is_bit_set` check (drawer).*
- **PC-7 (no in-bundle duplicates).** `bundle.spend` and `bundle.create` lists must each have unique UTXO keys. *Enforced by `no_duplicate_keys` in `process_bundle`.*
- **PC-8 (positive create amounts).** Every newly created UTXO has `amount > 0`. *Enforced by `assert_with_error!(amount > 0)` in both create paths.*
- **PC-9 (no duplicate addresses externally).** `op.deposit` and `op.withdraw` each have unique addresses. *Enforced by `verify_external_operations`.*
- **PC-10 (cross-side condition equality).** If an address appears in *both* `deposit` and `withdraw`, the two condition sequences must be XDR-equal. *Enforced by `verify_external_operations`.*
- **PC-11 (no condition conflicts).** Across the flat list of conditions in spend ∪ deposit ∪ withdraw, no pair conflicts under `Condition::conflicts_with`. *Enforced by `op_has_no_conflicting_conditions`.*
- **PC-12 (overflow safety on totals).** Summing `total_deposit` and `total_withdraw` uses `checked_add`; an overflow errors `AmountOverflow`. The internal supply uses `checked_add` / `checked_sub` via `treasury.rs`. *Enforced by `pre_process_channel_operation` and `treasury.rs`.*
- **PC-13 (depositor consent).** Every `ExtDeposit` requires `from.require_auth_for_args(vec![&e, conditions])`. The depositor cannot have funds pulled from their account without explicitly signing for the exact condition list. *Enforced by `transact.rs:122`.*
- **PC-14 (withdrawal authorization).** All withdrawals are authorized inside the bundle's `__check_auth` call (every spent UTXO's owner signed conditions covering the withdrawal). The contract itself self-authorizes the SAC transfer call via `authorize_as_current_contract`, which is sound only if `__check_auth` has already validated the bundle. *Enforced by ordering in `transact()` — pre_process and process_bundle precede `execute_external_operations`.*
- **PC-15 (atomicity).** The three phases (`pre_process`, `process_bundle`, `execute_external_operations`) execute within a single Soroban transaction; any panic reverts everything. *Implicit from Soroban semantics.*

### 4.3 Cross-contract invariants

- **X-1.** A Privacy Channel's `auth` address is set at construction and used for every bundle. Replacing the underlying Channel Auth requires replacing this address, which is not exposed externally and would require a contract `upgrade` to a new WASM that re-runs `set_auth`. *Enforced by code structure.*
- **X-2.** Multiple Privacy Channels may share the same Channel Auth instance. Compromise of a single Channel Auth admin therefore compromises every channel that names it. *Property, not a code-enforced invariant.*

---

## 5. Trust boundaries

The following diagram maps where data crosses trust boundaries and what must be verified at each crossing.

```
+--------------+        +-----------------+        +----------------+
|   User /     |        | Provider Plat-  |        |   Stellar /    |
|   Wallet     | ──→    | form (off-chain)| ──→    |   Soroban      |
+--------------+        +-----------------+        +----------------+
        │                      │                          │
        │ P256 sigs            │ Ed25519 sig              │ host crypto
        │ (UTXO conditions)    │ (bundle approval)        │ panics on bad sig
        │                      │                          │
        ▼                      ▼                          ▼
+--------------------------------------------------------------------+
|                       Channel Auth contract                        |
|                                                                    |
|  __check_auth:                                                     |
|    1. require_provider — Ed25519 verify against tx-auth payload    |
|       and registered provider set                                  |
|    2. handle_utxo_auth — P256 verify per UTXO context              |
+--------------------------------------------------------------------+
                              │
                              │ require_auth_for_args
                              ▼
+--------------------------------------------------------------------+
|                     Privacy Channel contract                       |
|                                                                    |
|  transact:                                                         |
|    pre_process     — structural / overflow / conflict checks       |
|    process_bundle  — atomic UTXO mutation + balance assertion      |
|    execute_external — SAC transfers + supply update                |
+--------------------------------------------------------------------+
                              │
                              │ TokenClient.transfer
                              ▼
                      +----------------+
                      |   Asset SAC    |
                      | (Stellar SAC,  |
                      |  e.g. XLM)     |
                      +----------------+
```

### 5.1 Boundary checklist

| Boundary | Direction | Required check | Where |
|---|---|---|---|
| User → Provider | off-chain | None on-chain. SDK responsibility. | n/a |
| Provider → Soroban host | tx submission | Stellar replay protection (sequence number on source account). | Stellar core |
| Soroban host → Channel Auth | `__check_auth` | Soroban auth-entry replay protection (per-account nonce, expiration ledger). | Soroban host |
| Channel Auth ↔ Privacy Channel | `require_auth_for_args` | Channel Auth must satisfy CA-1 .. CA-7. | `__check_auth` |
| Privacy Channel → Asset SAC (deposit) | sub-invocation | Depositor's `require_auth_for_args(conditions)` succeeds. | `transact.rs:122` |
| Privacy Channel → Asset SAC (withdrawal) | sub-invocation | Channel self-authorizes via `authorize_as_current_contract`; soundness requires `__check_auth` already passed. | `transact.rs:135-145` |
| Privacy Channel → UTXO storage | persistent read/write | Storage backend contracts: PC-5, PC-6, PC-8 enforced inside the storage layer. | `modules/storage/*` |

### 5.2 What is *not* a trust boundary

- The `modules/utxo-core` and `modules/auth` crates compile into the contract WASMs as `rlib` workspace deps. There is no inter-contract call or auth check between contract code and module code. They are part of the same trust domain as the contract that links them.
- The `modules/helpers::testutils` and `modules/utxo-core::testutils` paths are gated behind `#[cfg(feature = "testutils")]` and are not present in release WASM builds.

---

## 6. Out-of-scope notes

Auditors may reasonably assume the following are part of the audit; they are not.

- **The moonlight-sdk** (`Moonlight-Protocol/moonlight-sdk`): off-chain TypeScript code that derives keys, builds bundles, and constructs auth payloads. It produces the inputs to `transact` but is not part of this audit. Bugs in the SDK that cause it to construct unsafe operations are out of scope; the contracts must defend themselves regardless.
- **The provider-platform** (`Moonlight-Protocol/provider-platform`): off-chain Deno service that runs the mempool/executor/verifier loop and submits transactions on behalf of users. Out of scope.
- **The browser-wallet, council-console, provider-console, network-dashboard**: front-end applications. Out of scope.
- **The local-dev** orchestration repo: Docker Compose harness for E2E testing. Provides regression evidence (see `tests.md`) but its own code is not in audit scope.
- **The `contracts/token/` test token**: a workspace-internal token used only in `privacy-channel` integration tests. Not deployed.
- **The `admin-sep` crate** (`theahaco/admin-sep`): pulled in via git, rev `bf195f4d67cc96587974212f998680ccf9a61cd7`. Provides `Administratable` and `Upgradable` traits. Auditors should treat its behavior as part of the trusted base; if a deeper audit of `admin-sep` is desired, that is a separate scope.
- **The Soroban SDK fork** (`theahaco/rs-soroban-sdk`, rev `5a99659f1483c926ff87ea45f8823b8c00dc4cbd`): the workspace pins a specific revision of an Aha-maintained fork rather than `stellar/rs-soroban-sdk`. Where this fork diverges from upstream is itself worth review, but the SDK code is not in this audit scope. Auditors should be aware of the fork pin and may want to ask for the diff against upstream.
- **Stellar Asset Contract** behavior: the channel trusts its configured asset SAC to enforce its own transfer semantics. Custom non-SAC assets are not a tested path.

---

## 7. Toolchain and reproducibility

Captured here so readers do not need to grep the workspace.

- **Rust edition:** 2021 (workspace).
- **WASM target:** `wasm32v1-none` (Soroban-supported target; `release.yml` builds against this).
- **Soroban SDK:** git pin `https://github.com/theahaco/rs-soroban-sdk` rev `5a99659f1483c926ff87ea45f8823b8c00dc4cbd`.
- **`admin-sep`:** git pin `https://github.com/theahaco/admin-sep` rev `bf195f4d67cc96587974212f998680ccf9a61cd7`.
- **`stellar-default-impl-macro`:** git pin `https://github.com/OpenZeppelin/stellar-contracts` tag `v0.3.0`.
- **`wee_alloc`:** workspace dep version 0.4 (used as `#[global_allocator]` for `wasm32` builds).
- **Build command (CI):** `stellar contract build` (latest `stellar-cli`, locked install).
- **Release profile:** `opt-level = "z"`, `lto = true`, `codegen-units = 1`, `panic = "abort"`, `overflow-checks = true`, `strip = true`. Crucially, `overflow-checks` are enabled in release; arithmetic overflow panics rather than wrapping.

Per-contract Cargo features in use:

- `contracts/channel-auth/`: no features; default deps.
- `contracts/privacy-channel/`: links `moonlight-utxo-core` with `["no-utxo-events", "no-bundle-events"]` (event suppression). The `testutils` feature pulls testutils from utxo-core and Soroban SDK for unit tests.
- `modules/utxo-core/`: default features = `["storage-drawer", "no-utxo-events", "no-bundle-events"]`. The `storage-drawer` flag selects the bitmap-optimized backend over `storage-simple`.
- `modules/storage/`: provides `storage-simple` and `storage-drawer` as mutually exclusive features; selected by the consuming crate.

Workspace contract `version = "0.1.0"` per `Cargo.toml`; per-contract `Cargo.toml`s declare `version = "0.0.0"`. The workspace version is the one referenced by the `auto-tag.yml` workflow on push to `main`, which auto-creates `vX.Y.Z` git tags whenever `Cargo.toml` changes.
