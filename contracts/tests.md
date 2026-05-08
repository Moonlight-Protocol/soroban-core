# Soroban Core — Test Coverage and Testnet Evidence

This document describes the existing test surface for the in-scope contracts and identifies coverage gaps. It is intended to give the auditor a map of what has and has not been exercised, so the audit firm can scope their own coverage work accordingly.

The Audit Bank intake form asks for "previous security practices" and a remediation plan; we are explicit about gaps here rather than overclaiming.

---

## 1. Test layout

```
contracts/channel-auth/src/tests/
  mod.rs                  — test entry
  tests.rs                — end-to-end auth + UTXO transact path
  events.rs               — event emission assertions

contracts/privacy-channel/src/test/
  mod.rs                  — test entry
  test.rs                 — deposit + transfer happy paths
  channel_operation_builder.rs — test helper (not a test file itself)

modules/utxo-core/src/tests/
  mod.rs
  test.rs                 — UTXO accounting unit + bundle tests

modules/auth/src/test.rs  — provider/UTXO auth + signature verification

modules/helpers/src/tests.rs — strkey/Address roundtrip
```

All tests are Rust unit tests run via `cargo test` from the workspace root. There is no separate integration-test crate. The `local-dev` repo's Docker-based E2E suites exercise the deployed-contract surface; those are described in §4.

---

## 2. What each test covers

### 2.1 `contracts/channel-auth/`

#### `tests/tests.rs::test_auth_module`

Full happy-path validation of the auth contract acting as the authorization principal for a UTXO contract.

- Constructs `ChannelAuthContract` with a fresh admin (mocked Address).
- Constructs an in-test UTXO contract from `moonlight-utxo-core::testutils::contract` and binds it to the auth contract.
- Asserts `admin()` and `auth()` getters are correctly initialized.
- Adds a provider via `add_provider` with mocked admin auth; asserts `is_provider == true`.
- Mints two UTXOs via the test-only `mint` helper.
- Constructs a `UTXOOperation` that spends both UTXOs and creates two new ones.
- Each spend UTXO signs its conditions with its own P256 key.
- The provider signs the bundle's auth-entry payload hash with Ed25519.
- Submits via `client.set_auths(...)` and asserts both spent UTXOs are zero and both created UTXOs hold their expected balance.

**Invariants exercised:** CA-2 (provider threshold), CA-3 (expiration check, implicitly — uses `current + 1`), CA-4 (P256 coverage), CA-7 (event emission for `add_provider`, asserted in `events.rs`), PC-5/6/7 (UTXO uniqueness and dedup, indirectly via the underlying utxo-core).

#### `tests/events.rs`

Three event-emission tests:

- `test_constructor_emits_initialized_event` — asserts `contract_initialized(admin)` is the last event after construction.
- `test_add_provider_emits_event` — asserts `provider_added(provider)` after `add_provider`.
- `test_remove_provider_emits_event` — asserts `provider_removed(provider)` after `remove_provider`.
- `test_provider_lifecycle_with_events` — adds then removes a provider and asserts both `is_provider` transitions.

**Invariants exercised:** CA-7.

### 2.2 `contracts/privacy-channel/`

#### `test/test.rs::test_single_deposit_with_auth`

Single-depositor smoke. Adds a provider; mints a token balance to a Stellar G-account ("john"); deposits 500 tokens into the channel under the condition `Create(utxo_a, 500)`; asserts:

- `john`'s token balance decreased by 500.
- Channel `supply()` is 500.
- `utxo_a` balance is 500.

The deposit is signed by `john` via `sign_for_transaction` and the bundle is signed by `provider_a`. The flow constructs a real `xdr::SorobanAuthorizationEntry` for the depositor's auth, exercising the full Soroban auth-entry signing path (not `mock_all_auths`).

**Invariants exercised:** PC-3 (supply ↔ external flow), PC-4 (bundle balance with no spends), PC-13 (depositor consent), CA-2 (provider threshold), CA-4 (no P256 spends, but the AuthRequirements arg is empty so the check is a no-op).

#### `test/test.rs::test_auth_module`

Multi-deposit + multi-spend transfer lifecycle.

- Initializes channel + auth + token + admin.
- Asserts construction-time getters match (admin, auth, asset, supply == 0).
- Asserts `try_add_provider` without admin auth fails.
- Adds two providers under admin auth.
- Mints to `john` and `jane`; asserts balances.
- Deposits from both `john` (500 → 200+300 across utxo_a, utxo_c) and `jane` (600 → 300+300 across utxo_b, utxo_d) under matching `Create` conditions; asserts post-deposit balances match.
- Channel supply == 1100; UTXO balances match condition amounts.
- Sets ledger sequence to 3; constructs a transfer-only bundle (no deposit/withdraw) that spends all four UTXOs and creates five new ones with the original ones' conditions; asserts post-transfer balances and that supply is unchanged.

**Invariants exercised:** CA-1 (provider mutation gated on admin), CA-4 (P256 sigs for all four spends), PC-3 (supply unchanged on internal-only transfer), PC-4 (bundle balance), PC-5/6 (UTXO state transitions), PC-13 (depositor consent for both addresses).

**Coverage gap noted in this file:** the comment at `test.rs:96` says "no conditions as we cant properly test the G address signing mixed with the mocked address" — i.e. the team is aware that some auth-mixing scenarios remain difficult to exercise inside the Soroban test harness.

### 2.3 `modules/auth/`

#### `test.rs::test_auth_module`

Mirror of `contracts/channel-auth/tests/tests.rs::test_auth_module` but at the module level, against an in-test wrapper contract that surfaces `ProviderAuthorizable` and `UtxoAuthorizable`. Same invariants exercised.

#### `test.rs::test_auth_module_errors`

Negative-path test for the expiration check.

- Sets `e.ledger().set_sequence_number(10)`.
- Issues a signature with `expired_live_until_ledger = 9`.
- Submits; asserts `try_transact` returns the Soroban host's `Context::InvalidAction` error.
- Note: the comment at lines 167-175 acknowledges that the contract's structured `SignatureExpired` error is wrapped by the Soroban host's auth-failure handling, so the inner contract error is not directly observable from outside `__check_auth`. The test settles for asserting the outer `InvalidAction`.

**Invariants exercised:** CA-3.

#### `test.rs::test_ed25519_signatures`

Unit test for the `verify_signature` dispatch. Generates an Ed25519 keypair, signs a known payload, populates a `SignerKey::Ed25519 → (Signature::Ed25519, u32::MAX)` map, and round-trips through `verify_signature`. Mostly validates the mapping pattern in `core::verify_signature` rather than auth flow proper.

### 2.4 `modules/utxo-core/`

#### `tests/test.rs::test_mint_and_burn`

UTXO state machine fundamentals:

- Non-existing UTXO returns -1.
- After mint, balance equals the minted amount.
- After burn, balance is 0.
- Re-minting an existing UTXO errors `UTXOAlreadyExists`.
- Burning an already-spent UTXO errors `UTXOAlreadySpent`.
- Burning a never-existed UTXO errors `UTXODoesntExist`.
- Minting with negative amount errors `InvalidCreateAmount`.
- Minting with zero amount errors `InvalidCreateAmount`.

**Invariants exercised:** PC-5, PC-6, PC-8.

#### `tests/test.rs::test_transfer`

Full bundle lifecycle:

- Mint utxo_a 250.
- Transfer (spend utxo_a, create utxo_b 250) succeeds; balances flip.
- Bundle with duplicate creates errors `RepeatedCreateUTXO`.
- Bundle with duplicate spends errors `RepeatedSpendUTXO`.
- Unbalanced bundle (550 in, 500 out) errors `UnbalancedBundle`.
- Balanced bundle (550 in, 550 out across multiple creates) succeeds.

**Invariants exercised:** PC-4, PC-7.

#### `tests/test.rs::test_transfer_auth_mocked`

Auth-failure paths around the `auth.require_auth_for_args` site:

- Bundle with valid structure but no auth → outer `Context::InvalidAction`.
- Bundle with a `set_auths` entry but a `false` mock signature → outer `Context::InvalidAction`.
- Bundle with `true` mock signature → succeeds.

**Invariants exercised:** CA-4 surrogate (the actual P256 check is mocked here; this test verifies the Soroban auth-entry plumbing).

#### `tests/test.rs::test_transfer_with_external`

Tests the `transact_with_external(op, additional_in, additional_out)` test-only entry point that exposes the bundle-balance contract directly, without going through the Privacy Channel's `transact`. Validates that:

- A bundle that needs additional_in but is given 0 → `UnbalancedBundle`.
- A bundle that needs additional_out but is given 0 → `UnbalancedBundle`.
- Passing the correct values → succeeds.

**Invariants exercised:** PC-4 (bundle balance equation with non-zero deposit/withdraw totals).

#### `tests/test.rs::test_transfer_with_additional_auth_conditions`

Tests that a bundle with `Condition::ExtDeposit`, `ExtWithdraw`, and `ExtIntegration` entries inside its spend conditions affects the auth requirements (the `AuthRequirements` map keyed by P256 signers includes those conditions) but does not affect the balance equation. The test uses `transact_with_external` to supply the deposit/withdraw totals manually since this test is at the utxo-core layer, not the Privacy Channel layer.

**Invariants exercised:** auth payload construction includes Ext* conditions; balance equation is independent of auth-conditions content.

### 2.5 `modules/helpers/`

#### `tests.rs::roundtrip_pk_to_address_and_back`

Asserts `address_from_ed25519_pk_bytes` and `address_to_ed25519_pk_bytes` are mutually inverse for a hardcoded 32-byte key.

#### `tests.rs::address_string_roundtrip_works`

Same, starting from a `Strkey` string.

#### `tests.rs::start_from_address_string`

Same, starting from a known `G…` Stellar address string.

These are correctness checks for the strkey conversion path used by `require_provider` to look up provider addresses.

---

## 3. Coverage gaps

The following invariants from `arch.md` are *not* directly exercised by the existing test suite, or are exercised only indirectly. We document them honestly here; remediation (closing these gaps) is out of scope for this PR and is dispatched as a separate prompt.

### 3.1 Direct gaps in invariant coverage

| Invariant | Gap |
|---|---|
| **CA-2 (provider threshold)** | Only the success path is tested. There is no negative test for *zero* provider signatures (i.e., a bundle submitted with only P256 sigs and no provider sig should error `ProviderThresholdNotMet`). Worth adding. |
| **CA-3 (expiration)** | Tested for the all-signatures-expired case in `auth/test.rs::test_auth_module_errors`. Not tested for *partial* expiration (e.g., one P256 sig expired but provider sig fresh, or vice versa). Worth adding. |
| **CA-5 (context shape)** | No test exercises a non-`Contract` context to verify `UnexpectedContext` is raised. Likely difficult to set up via the Soroban test harness. |
| **CA-6 (signature/key match)** | No test mismatches a P256 SignerKey with an Ed25519 Signature (or vice versa) to verify `InvalidSignatureFormat`. Worth adding. |
| **PC-1 (immutable asset binding)** | No test attempts to overwrite the asset address post-construction. Code structure does not expose a setter, so the gap is structural rather than behavioral, but a regression test guarding against a future `set_asset` accidentally being added would be cheap. |
| **PC-2 (immutable auth binding)** | Same as PC-1: no setter exposed; no regression test. |
| **PC-9 (no duplicate addresses externally)** | No test submits a `ChannelOperation` with two `(Address, _, _)` deposit entries for the same address; the `RepeatedAccountForDeposit` and `RepeatedAccountForWithdraw` errors do not have direct test coverage. |
| **PC-10 (cross-side condition equality)** | The test-only `ChannelOperationBuilder` enforces this via `panic!` at construction time (`channel_operation_builder.rs:77-82`), but the on-chain `ConflictingConditionsForAccount` error itself is not exercised by a test that submits to `transact`. |
| **PC-11 (no condition conflicts)** | `BundleHasConflictingConditions` error has no direct test that submits a bundle with `Create(u, 100)` on one spend and `Create(u, 200)` on another. |
| **PC-12 (overflow safety on totals)** | `AmountOverflow` is not exercised. Hard to trigger naturally (requires `i128::MAX`-class amounts), but a unit test with crafted inputs would document the path. |
| **Drawer rotation** | The bitmap drawer (`storage-drawer`) tests rotate at 1024 slots. No test pushes the slot allocator past `SLOTS_PER_DRAWER` to exercise the rotation+flush path in `alloc_slot_and_rotate_if_needed_cached`. The team should consider a stress test minting >1024 UTXOs in a session. |
| **Storage backend swap** | All tests run with the workspace-default backend (`storage-drawer`). The `storage-simple` backend is compile-checked but not exercised end-to-end. |
| **Upgrade path** | `Upgradable::upgrade(wasm_hash)` from `admin-sep` is not exercised by any in-tree test. The pattern is admin-gated and inherited from a trusted base, but the lack of a regression test means any locally-introduced override would not be caught. |
| **`set_admin` event** | There is no event emitted on `set_admin`; this is documented as an invariant gap in arch §4.1, not a test gap. |

### 3.2 Test-mocking caveats

- Most provider-add and admin-action tests use `mock_auths` rather than constructing real Stellar auth entries. This is appropriate for unit testing but means the auth-entry XDR shape is only exercised through the Soroban test framework's mocking layer, not against a real Stellar host. The `privacy-channel::test_single_deposit_with_auth` and `test_auth_module` tests do construct real auth entries for the *deposit* path (via `xdr::SorobanAuthorizationEntry`), which is the most production-realistic surface in the suite.
- The provider-only auth path against a real auth-entry XDR (no `mock_auths`) is exercised by the multi-utxo cases in `auth/test.rs::test_auth_module` and `channel-auth/tests/tests.rs::test_auth_module`.
- The Soroban host wraps inner `__check_auth` errors in `ScErrorType::Context, ScErrorCode::InvalidAction`, so the existing tests cannot directly assert which inner contract error fired. The team's `test_auth_module_errors` comment (`auth/test.rs:167-175`) acknowledges this.

### 3.3 Coverage tools not yet wired

- No `cargo tarpaulin` / `cargo-llvm-cov` line-coverage report is generated by CI. There is no enforced minimum coverage threshold. Adding line-coverage measurement is a small lift and would help future-proof the suite; out of scope here.
- No fuzz testing or property-based testing. The bundle-balance equation (PC-4) is a natural target for property-based testing (`proptest`); the condition-conflict logic is another. Out of scope.

---

## 4. Testnet evidence

Per SCF #37 D1 (delivered 2026-03-18 with `soroban-core` v0.1.0), both contracts are deployed on Stellar **testnet** with the following IDs:

| Contract | Testnet contract ID |
|---|---|
| Channel Auth | `CAF7DFHTPSYIW5543WBXJODZCDI5WF5SSHBXGMPKFOYPFRDVWFDNBGX7` |
| Privacy Channel | `CDMZSHMT2AIL2UG7XBOHZKXM6FY3MUP75HAXUUSAHLGRQ2VWPGYKPM5T` |

Verification of these IDs is captured separately in `testnet-readiness.md`.

### 4.1 End-to-end exercise outside cargo test

Beyond the in-repo unit tests, the contracts are exercised end-to-end by the Moonlight `local-dev` repository:

- **E2E suite** (`local-dev/.github/workflows/e2e-reusable.yml`): consumed by this repo's `pr.yml` workflow on every pull request. The `e2e` job runs against the freshly built contract WASMs uploaded as the `contract-wasms` artifact and exercises the full provider-platform → channel deposit/transfer/withdraw lifecycle.
- **Lifecycle suite** (`local-dev/.github/workflows/lifecycle-reusable.yml`): also consumed by `pr.yml`. Validates a longer-running deposit → transfer → withdraw cycle including provider-platform restart and observability checks.
- **Cross-repo dispatch** (`release.yml`): on tag push (auto-tag triggers on `Cargo.toml` change), the WASMs are released to GitHub and a `repository_dispatch` is sent to `local-dev` with `event-type: module-release`, which kicks off integration tests across the rest of the stack against the new release.

Net effect: every commit to `main` of `soroban-core` runs through E2E and lifecycle suites against a Dockerized stack that includes the provider-platform, browser-wallet, and a real local Stellar instance. This is the strongest evidence we have that the contracts are exercised end-to-end against realistic conditions.

### 4.2 Public testnet console references

The deployed testnet contracts are also referenced from the public-facing consoles:

- Council Console: `https://moonlight-council-console.fly.storage.tigris.dev`
- Network Dashboard: `https://network-dashboard.fly.storage.tigris.dev`

These are out of audit scope but are linked here so the auditor can independently inspect the live testnet state.

---

## 5. Tooling and reproducibility

To re-run the contract test suite locally against this repo:

```
git clone https://github.com/Moonlight-Protocol/soroban-core
cd soroban-core
cargo test --workspace
```

Build the deployable WASMs with:

```
stellar contract build
```

The CI build (`pr.yml`) uses:

- Rust toolchain: `dtolnay/rust-toolchain@stable`, target `wasm32v1-none`.
- Soroban CLI: `cargo install --locked --force stellar-cli` (latest).
- System deps: `pkg-config libdbus-1-dev libudev-dev` (Ubuntu).

Per-test runtime is dominated by the Soroban test-host setup; the full workspace test suite typically completes in under a minute on a modern laptop.

Versions and pin hashes are documented exhaustively in `arch.md` §7.
