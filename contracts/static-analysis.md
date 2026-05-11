# Soroban Core — Self-Service Static Analysis

This document satisfies item 5 of the Soroban Audit Bank intake form ("self-service tooling vulnerability scan results, with remediation plan for any findings"). It captures the tools run, the exact commands used, the raw findings, and the per-finding remediation plan.

The audit scope is the two contracts in `contracts/` plus the `modules/` they compile against. The `contracts/token/` test-only token is excluded from the security plan but still scanned by the workspace-wide commands.

---

## 1. Tools and versions

| Tool | Version | Purpose |
|---|---|---|
| `rustc` | 1.94.0 (4a4ef493e 2026-03-02) | Toolchain |
| `cargo` | 1.94.0 (85eff7c80 2026-01-15) | Build / test driver |
| `cargo-audit` | 0.22.1 | RustSec advisory scan over `Cargo.lock` |
| `cargo-clippy` | 0.1.94 (4a4ef493e3 2026-03-02) | Static lint pass with the `pedantic` group enabled |

The `stellar-cli` itself is also part of the build pipeline (`stellar contract build`) but does not currently expose an independent static-analysis surface beyond what `cargo` already provides; we did not invoke it as a static-analysis tool.

We did not run a Soroban-specific commercial linter (e.g. CoinFabrik's `cargo-scout-audit`) for this self-service pass; the Audit Bank form requires "results of one of the self-service tooling options" and the combination of `cargo-audit` + `cargo-clippy` with the pedantic group is widely accepted as that minimum bar. The Audit Bank firm assigned to this engagement may wish to run their own Soroban-specific tooling on top.

---

## 2. `cargo audit` — RustSec advisory scan

### 2.1 Command

```
cd soroban-core
cargo generate-lockfile        # workspace tracks Cargo.toml only; Cargo.lock generated for scan
cargo audit
```

### 2.2 Result summary

```
Scanning Cargo.lock for vulnerabilities (192 crate dependencies)
warning: 3 allowed warnings found
```

**Vulnerabilities: 0.** **Unmaintained advisories: 3.**

### 2.3 Findings

#### F-AUDIT-1 — `derivative 2.2.0` (unmaintained)

- Advisory: RUSTSEC-2024-0388 — https://rustsec.org/advisories/RUSTSEC-2024-0388
- Severity: warning (informational; "unmaintained crate"). No known CVE.
- Dependency path: pulled in transitively via the Soroban SDK chain (specifically `soroban-env-host` → `wasmi`/`ark-ec` graph). Not a direct dependency of either in-scope contract.

**Remediation plan:** **Out of scope for this PR; accept-and-track.**

The dependency arrives through the Soroban SDK pin (`theahaco/rs-soroban-sdk`, rev `5a99659f…`). Replacing or upgrading `derivative` would require either an upstream upgrade in the Soroban SDK or a fork. The advisory is informational; the crate has no known security vulnerability — the maintainer simply stopped accepting changes. We will track upstream SDK updates and pick up a fix whenever the Soroban SDK does. No on-chain risk to the contracts in the meantime.

#### F-AUDIT-2 — `paste 1.0.15` (unmaintained)

- Advisory: RUSTSEC-2024-0436 — https://rustsec.org/advisories/RUSTSEC-2024-0436
- Severity: warning (informational; "no longer maintained"). No known CVE.
- Dependency path: `paste` ← `wasmi_core 0.13.0` ← `soroban-wasmi 0.31.1-soroban.20.0.1` ← `soroban-env-host 23.0.0-rc.2` ← `soroban-sdk`. Used as a procedural-macro helper inside the Soroban runtime stack.

**Remediation plan:** **Out of scope for this PR; accept-and-track.** Same disposition as F-AUDIT-1 — pulled in transitively via the Soroban SDK. The crate is widely used and has no exploitable advisory; remediation is gated on the Soroban SDK adopting an alternative.

#### F-AUDIT-3 — `wee_alloc 0.4.5` (unmaintained)

- Advisory: RUSTSEC-2022-0054 — https://rustsec.org/advisories/RUSTSEC-2022-0054
- Severity: warning (informational; "unmaintained"). No known CVE.
- Dependency path: **direct** dependency of both `privacy-channel` and `channel-auth-contract`. Used as the `#[global_allocator]` for `wasm32` builds:

  ```rust
  #[cfg(target_arch = "wasm32")]
  #[global_allocator]
  static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;
  ```

  in both `contracts/channel-auth/src/lib.rs` and `contracts/privacy-channel/src/lib.rs`.

**Remediation plan:** **Track for follow-up; not blocking the audit.**

`wee_alloc` is a small-footprint allocator widely used in the Soroban / Stellar contract ecosystem to keep WASM code size small. The advisory flags maintenance status, not a known vulnerability. Replacing it would require:

1. picking a maintained alternative (`dlmalloc`, the default Rust allocator, or `lol_alloc`),
2. validating the WASM size budget remains within Soroban contract limits,
3. validating that resulting gas costs do not regress.

This is a self-contained change but represents a non-trivial regression-test surface; it does not block audit readiness and should be scheduled as a separate PR after the Audit Bank engagement completes (so that any fix can be applied alongside the audit firm's findings rather than re-shuffling auditable code immediately before audit).

### 2.4 Net audit-form summary

- **0 known vulnerabilities** in the dependency graph.
- **3 unmaintained-crate advisories** — 2 transitive (out of our control), 1 direct (`wee_alloc`, scheduled).
- **0 of the findings represent an exploitable on-chain risk** at the contract layer.

---

## 3. `cargo clippy` — pedantic lint pass

### 3.1 Command

```
cd soroban-core
cargo clippy --workspace --no-deps --all-targets -- \
    -W clippy::all \
    -W clippy::pedantic
```

`--no-deps` ensures we only lint our own sources; `--all-targets` includes test code.

### 3.2 Result summary

- **0 errors.**
- **307 warnings.**
- Top categories:

| Count | Category | Severity for security audit |
|---|---|---|
| 89 | `needless_borrow` (immediately-dereferenced reference) | Style |
| 32 | `must_use_candidate` (method) | Style |
| 30 | `needless_pass_by_value` | Style |
| 19 | `use_self` / `use_infallible_conversion` | Style |
| 17 | `missing_panics_doc` | **Audit-relevant** — see §3.3 |
| 12 | `must_use_candidate` (function) | Style |
| 11 | `clone_on_copy` (`u32`) | Style |
| 11 | `if_then_panic` (single-statement-panic in `if`) | Style |
| 10 | `assert_eq` with literal bool | Style (test-only) |
| 6 | `inline_always` on small fns | Style |
| Misc | various | Style |

The full output is reproducible by running the command above; we do not duplicate the 3,556-line raw output in this document.

### 3.3 `missing_panics_doc` — 17 findings (audit-relevant)

These flag functions that may panic without a `# Panics` rustdoc section. Each one corresponds to an undocumented termination path. None of the panic paths are exploitable in the sense that they bypass authorization, but each is a deliberate revert that auditors would expect to see documented. The findings cluster as follows:

| File | Function | Reason for panic |
|---|---|---|
| `modules/helpers/src/parser.rs` | `address_to_ed25519_pk_bytes` | Invalid UTF-8, invalid Strkey, non-ed25519 address (testutils-only path). |
| `modules/storage/src/simple.rs` | `SimpleStore::create`, `SimpleStore::spend` | Already-exists / already-spent / not-exists (intentional revert). |
| `modules/storage/src/drawer.rs` | `DrawerStore::create_cached`, `spend_cached`, `alloc_slot_and_rotate_if_needed_cached` | Already-exists / already-spent / drawer-id u32 overflow (last is unreachable in practice). |
| `modules/utxo-core/src/core.rs` | `process_bundle`, `verify_utxo_not_exists`, `verify_utxo_unspent`, `auth` | Bundle-balance / UTXO-state / missing-auth-config reverts. |
| `modules/auth/src/core.rs` | `register_provider`, `deregister_provider`, `require_provider` | Provider-already / not-registered / signature checks. |
| `contracts/privacy-channel/src/transact.rs` | `pre_process_channel_operation`, `verify_external_operations`, `op_has_no_conflicting_conditions` (via `panic_with_error!`) | Bundle-shape reverts. |
| `contracts/privacy-channel/src/treasury.rs` | `increase_supply`, `decrease_supply` | Overflow / underflow. |

**Remediation plan:** **Documentation pass; not a code change.** Adding `# Panics` doc-comment sections to every flagged function is straightforward and entirely additive. Out of scope for this PR; tracked as a follow-up cleanup. Auditors can treat the panic semantics as the documented intent: any bundle-shape, UTXO-state, supply, or auth failure causes a transaction-level revert.

### 3.4 Style-only findings (audit-irrelevant)

The remaining 290 warnings are stylistic — `needless_borrow`, `must_use_candidate`, `clone_on_copy`, etc. None of them indicates a security vulnerability. They are tracked as a separate cleanup batch and are not blocking the audit.

---

## 4. Manual code-walk observations

In addition to the tool output, the team conducted a manual review of the in-scope sources during this exercise. The observations below are not findings against an automated tool's rule but are surfaced here for the auditor's attention. They are NOT presented as defects; they are informational notes that may shape the auditor's own threat model.

### 4.1 Signature-verifier wrappers rely on host-panic semantics

`modules/auth/src/core.rs`:

```rust
fn verify_p256_signature(...) -> Result<(), Error> {
    e.crypto().secp256r1_verify(public_key, payload_hash, signature);
    Ok(())
}

fn verify_ed25519_signature(...) -> Result<(), Error> {
    e.crypto().ed25519_verify(public_key, &Bytes::from_array(...), signature);
    Ok(())
}
```

Both wrappers always return `Ok(())`. They depend on the underlying Soroban host primitives panicking on a failed verification. This is the documented behavior of `secp256r1_verify` and `ed25519_verify` in the Soroban SDK at the pinned revision (`5a99659f…` of `theahaco/rs-soroban-sdk`).

**Implication:** if the upstream SDK ever changes these primitives to return a `Result` rather than panic, the wrappers as written would silently accept invalid signatures. This is not a current vulnerability but is a non-obvious coupling worth documenting and worth re-validating against any future SDK upgrade.

**Remediation plan:** **Track for follow-up.** Wrappers should be hardened to defensively re-check the host's behavior or return a `Result` from the host primitive. Out of scope for this PR.

### 4.2 Zero-arg context bypass in `handle_utxo_auth`

`modules/auth/src/core.rs:87`:

```rust
if cc.args.len() < 1 {
    return Ok(()); // No auth requirements, skip
}
```

Zero-argument contract contexts skip the per-UTXO P256 verification entirely. In the current call graph this is reached only from `process_bundle` when `bundle.req.0.is_empty()` (i.e., no spends), and provider authorization (which runs first and unconditionally requires a registered provider sig) still applies. **No exploit path is currently reachable.**

**Implication:** the safety of this implicit "zero-args means no spends" coupling depends on the contract layer never building an `auth_args` of `vec![&e]` for a bundle that *does* have spends. A future refactor could break this. Worth a `# Safety` comment at minimum.

**Remediation plan:** **Track for follow-up.** Consider tightening to: if `contexts` references a contract function that is expected to carry auth, *require* a non-empty arg list explicitly, rather than implicitly accepting empty-args as success.

### 4.3 Provider threshold is hardcoded to 1

`modules/auth/src/core.rs:205`:

```rust
const PROVIDER_THRESHOLD: u32 = 1; // For now we require only one exact provider signature
```

Acceptable for the current architecture (channels are bound to a single Channel Auth contract whose admin governs the provider set; quorum semantics live above this layer). Worth flagging for the auditor since the const-named "threshold" pattern usually invites questions about multi-sig / m-of-n; here it is intentional.

**Remediation plan:** **No change.** Documented in arch.md §2.5. If a future product requirement demands m-of-n provider quorums, this becomes a configurable storage value; that's a roadmap decision, not a fix.

### 4.4 `panic!` vs `panic_with_error!` inconsistency

`contracts/privacy-channel/src/treasury.rs`:

```rust
None => panic!("Overflow occurred while increasing supply"),
None => panic!("Underflow occurred while decreasing supply"),
```

Other revert paths in the codebase use `panic_with_error!(env, Error::SomeVariant)` which surfaces a structured contract-error code to the host. The two `panic!` calls in `treasury.rs` produce a string panic instead, which loses the structured-error mapping.

**Implication:** if `transact` triggers an internal supply overflow / underflow, the resulting transaction failure is harder to programmatically diagnose from the client side. The path is still rejected; only the diagnosability is degraded.

**Remediation plan:** **Track for follow-up.** Add a `treasury::Error` enum and convert these panics to `panic_with_error!`. Out of scope for this PR.

### 4.5 Production `unwrap()` sites

| Location | Context |
|---|---|
| `contracts/privacy-channel/src/storage.rs:20` (`read_asset`) | Constructor sets it; impossible-by-construction. |
| `modules/auth/src/core.rs:99` | Map key was just iterated from same map; impossible-by-construction. |
| `modules/auth/src/core.rs:218` | `.ok_or(MissingSignature).unwrap()` — would be more consistent as `?` propagation. |
| `modules/storage/src/drawer.rs:104` | Just-set `Option<DrawerState>::Some(_)`; impossible. |
| `modules/utxo-core/src/core.rs:68` (`auth`) | Constructor sets it; impossible-by-construction. |
| `modules/helpers/src/parser.rs:31` | Bare `panic!` in `address_to_ed25519_pk_bytes`. testutils-only path. |

**Remediation plan:** **Track for follow-up.** None of these is a current bug. They could each be tightened either by `expect("…")` with a precondition message or by `?` / structured error propagation. Out of scope for this PR.

### 4.6 Privacy Channel emits no contract events

`contracts/privacy-channel/Cargo.toml` enables `no-utxo-events` and `no-bundle-events` on `moonlight-utxo-core`. The Privacy Channel itself does not declare any `#[contractevent]` types. The only on-chain event traffic from `transact` comes from the asset SAC's own `transfer` events.

**Implication:** for purely internal `Spend`/`Create` transactions (no `ExtDeposit` / `ExtWithdraw`), there is no contract-level event emitted. The transaction's existence is observable via Stellar transaction metadata, fees, and resulting UTXO storage state, but there is no per-bundle audit-trail event on-chain.

This is a **deliberate cost optimization** — bundle and per-UTXO events are expensive when bundles batch many UTXOs — but it is also a **repudiation surface**: an external observer cannot reconstruct the bundle's contents without inspecting the TX footprint. Privacy is a feature, not a bug, in this domain (this is a privacy channel by design); but auditors should be aware that the on-chain audit trail is intentionally minimal.

**Remediation plan:** **No change planned.** Documented in `arch.md` §3.4 and revisited in `stride.md` under the Repudiation category.

### 4.7 Strict `<` for `valid_until_ledger`

`modules/auth/src/core.rs:113, 220`:

```rust
if valid_until_ledger < e.ledger().sequence() {
    return Err(Error::SignatureExpired);
}
```

A signature with `valid_until_ledger == current_sequence` is **still valid** (the comparison is strict-less-than). This is consistent with a "live until and including ledger N" interpretation. Documented here so the auditor does not flag it as an off-by-one.

**Remediation plan:** **No change planned.** Documented as intentional.

---

## 5. Combined remediation plan

| ID | Finding | Disposition | Tracked-as |
|---|---|---|---|
| F-AUDIT-1 | `derivative` unmaintained (transitive) | Accept-and-track | Future Soroban SDK upgrade |
| F-AUDIT-2 | `paste` unmaintained (transitive) | Accept-and-track | Future Soroban SDK upgrade |
| F-AUDIT-3 | `wee_alloc` unmaintained (direct) | Track for follow-up | Post-audit cleanup PR |
| F-CLIPPY-1 | 17× `missing_panics_doc` | Track for follow-up | Doc-only PR |
| F-CLIPPY-2 | 290× style-only warnings | Defer | Project-wide cleanup |
| F-MANUAL-1 | Verifier wrappers rely on host panic | Track for follow-up | Hardening PR |
| F-MANUAL-2 | Zero-arg context bypass | Track for follow-up | Hardening PR |
| F-MANUAL-3 | Hardcoded provider threshold | No change | Documented intent |
| F-MANUAL-4 | `panic!` vs `panic_with_error!` inconsistency in `treasury.rs` | Track for follow-up | Cleanup PR |
| F-MANUAL-5 | Production `unwrap()` sites | Track for follow-up | Cleanup PR |
| F-MANUAL-6 | No contract events on internal-only `transact` | No change | Documented intent (privacy by design) |
| F-MANUAL-7 | Strict-`<` on `valid_until_ledger` | No change | Documented intent |

**Summary for the Audit Bank readiness reviewer:** zero exploitable findings, three accept-and-track unmaintained-crate advisories, seven informational manual notes that the audit firm may want to look at independently. No critical / high / medium findings to remediate before audit.
