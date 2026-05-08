# Soroban Core — STRIDE Threat Model

This document satisfies item 6 of the Soroban Audit Bank intake form ("STRIDE threat model for the contracts"). It enumerates threats per STRIDE category for both in-scope contracts. Each entry lists threat scenario, likelihood, impact, mitigation already in place, and residual risk after mitigation.

The model is scoped to the on-chain contract layer only. Off-chain components (provider platform, browser wallet, council/provider/network consoles, moonlight-sdk) and the Stellar/Soroban host itself are treated as part of the trust environment and are not enumerated here. Where their behavior is load-bearing for a contract-level mitigation, that dependency is called out explicitly.

Likelihood and impact ratings are intentionally coarse:

- **Likelihood:** Low / Medium / High — operational chance of the scenario being attempted in the wild given current architecture.
- **Impact:** Low / Medium / High / Critical — worst-case effect on protocol integrity, user funds, or auditability.
- **Residual risk** is the post-mitigation rating.

Cross-references to invariants (CA-1..CA-7, PC-1..PC-15, X-1..X-2) are to `arch.md`.

---

## 1. Channel Auth

### 1.1 Spoofing

| ID | Threat | Likelihood | Impact | Mitigation | Residual |
|---|---|---|---|---|---|
| CA-S-1 | An attacker submits a transaction claiming to be a registered provider, attaching a forged Ed25519 signature. | High (every adversary attempts) | Critical (would let any attacker authorize bundles) | `require_provider` (CA-2): the Soroban host's `ed25519_verify` panics on invalid signatures; the registered-provider lookup uses an instance-storage map; `address_from_ed25519_pk_bytes` derives the address from the pubkey deterministically, so an attacker cannot supply a public key whose strkey matches a registered provider without holding the secret. | Low |
| CA-S-2 | Attacker spoofs the *contract address* itself (e.g. via a malicious contract impersonating Channel Auth in front of the Privacy Channel). | Low | High | The Privacy Channel stores the Channel Auth address in instance storage at construction (`STORAGE_KEY_UTXO_AUTH`) with no exposed setter (PC-2). Replacing it would require an admin-gated contract upgrade. | Low |
| CA-S-3 | Attacker spoofs an admin call by predicting / racing the admin's transaction. | Low | High | Admin authorization is enforced by Soroban's `require_auth`; the admin's signature is bound to network ID, contract, function name, args, sequence, expiration. There is no in-contract replay window beyond Soroban's own auth-entry nonce semantics. | Low |
| CA-S-4 | Attacker spoofs a P256 UTXO signer by deriving a colliding P256 pubkey. | Low (cryptographic) | Critical | `secp256r1_verify` host primitive; SEC1 uncompressed pubkey (`BytesN<65>`) on-curve validation by the host. Collision security is the standard NIST P-256 / secp256r1 ~128-bit floor. | Low |
| CA-S-5 | Attacker submits a P256 signature that verifies against the wrong message (replay across contracts). | Low | High | Per-UTXO auth payload includes the caller contract's address bytes via `hash_payload(payload, contract_addr_bytes)` (CA-4). Cross-contract replay is bound to a different message digest. | Low |

### 1.2 Tampering

| ID | Threat | Likelihood | Impact | Mitigation | Residual |
|---|---|---|---|---|---|
| CA-T-1 | Attacker tries to add themselves to the provider set without admin auth. | High | Critical | `add_provider` calls `Self::require_admin(e)` first; admin-sep traits enforce admin signature presence. (CA-1) | Low |
| CA-T-2 | Attacker tries to remove a competing provider, denying that provider its bundle-signing role. | Medium | Medium | Same as CA-T-1: admin-gated. (CA-1) | Low |
| CA-T-3 | Attacker forges an `AuthRequirements` argument to bypass per-UTXO P256 verification. | Medium | High | `AuthRequirements` is parsed from `args[0]` via `try_into_val`; type mismatches error `BadArg`. The map's keys are `SignerKey::P256(pk)` and the verification routine recomputes the payload over the *same* conditions before checking. An attacker who tampers with the requirements map can only either (a) drop a P256 entry (in which case the corresponding spend's `MissingSignature` error fires), (b) add an extraneous entry (silently skipped if non-P256, or fails verification if P256 with no real signer). The map cannot be tampered with to authorize a UTXO whose owner did not sign. (CA-4) | Low |
| CA-T-4 | Attacker tampers with `valid_until_ledger` to extend a stale signature's lifetime. | Medium | High | `valid_until_ledger` is signed-over as part of the auth payload (`hash_payload(AuthPayload { ..., live_until_ledger })`). Modifying it changes the payload and invalidates the signature. (CA-3, CA-4) | Low |
| CA-T-5 | Storage tampering — attacker mutates `ProviderDataKey::AuthorizedProvider(addr)` directly. | Low | Critical | Soroban's instance storage is contract-scoped; only this contract's code can write to it. There is no public storage-mutator function. | Low |

### 1.3 Repudiation

| ID | Threat | Likelihood | Impact | Mitigation | Residual |
|---|---|---|---|---|---|
| CA-R-1 | Admin denies having added or removed a particular provider. | Low | Medium | `ProviderAdded` and `ProviderRemoved` events are emitted at every governance action (CA-7), bound to the registered provider's address. Combined with the Stellar transaction record showing the signed admin auth-entry, this provides a non-repudiable trail. | Low |
| CA-R-2 | Admin denies having upgraded the contract. | Medium | Medium | The contract does not emit a dedicated `ContractUpgraded` event (gap noted in arch §4.1). However, contract upgrade is observable on-chain via the Stellar transaction itself (the `upload_contract_wasm` / `extend_contract` operations are public ledger entries); admin-sep produces internal hooks that may emit. The audit trail exists but is shallower than for provider mutations. | **Medium** |
| CA-R-3 | Admin denies having transferred admin rights. | Medium | Medium | Same as CA-R-2: no dedicated event from this contract. The `set_admin` call is on-chain and visible in the transaction record, but there is no in-contract signal. | **Medium** |
| CA-R-4 | Provider denies having signed a particular bundle. | Low | Medium | The provider's Ed25519 signature is part of the Soroban auth-entry XDR captured in the transaction's metadata. Cryptographic non-repudiation is full. | Low |

### 1.4 Information disclosure

| ID | Threat | Likelihood | Impact | Mitigation | Residual |
|---|---|---|---|---|---|
| CA-I-1 | Provider set is publicly readable via `is_provider`. | Certain (by design) | None — public by design | Provider membership is a public protocol fact; the read is intentional. | None |
| CA-I-2 | Admin address is publicly readable via `admin()`. | Certain | None — public by design | Same: governance principal is intended to be public. | None |
| CA-I-3 | `__check_auth` accepts a `signatures` map that may leak signer identities to a passive observer. | Certain | Low — by design | All auth-entry data is on the public ledger. The protocol does not attempt to hide signer identity at the Channel Auth layer; UTXO ownership privacy is handled at the Privacy Channel layer (where UTXOs are unlinkable from Stellar accounts). | None |

### 1.5 Denial of service

| ID | Threat | Likelihood | Impact | Mitigation | Residual |
|---|---|---|---|---|---|
| CA-D-1 | Attacker submits a bundle with an enormous signatures map to exhaust gas / instructions during `__check_auth`. | Medium | Medium | The transaction submitter pays the resource fee; Soroban metering bounds per-tx execution. The contract iterates `signatures.0.keys()` once per provider check and once per UTXO context, both linear in map size. There is no quadratic blowup. The economic cost falls on the attacker, who gains nothing. | Low |
| CA-D-2 | Attacker spams `add_provider` / `remove_provider` to bloat instance storage and increase TTL extension cost for the admin. | Low | Low | Admin-gated; only the admin can pay this cost and they would be paying it on themselves. Not a viable external attack vector. | Low |
| CA-D-3 | Provider key compromise leads to attacker griefing all bundles by submitting expired or otherwise-invalid sigs that cost everyone gas. | Medium | Medium | Provider Ed25519 verification is host-native and cheap; failed bundles are reverted and the attacker pays the resource fee. The admin can `remove_provider` to evict a compromised provider. | Low |
| CA-D-4 | A `Context` variant other than `Contract` is supplied to attempt to exhaust `handle_utxo_auth`. | Low | Low | Rejected immediately with `UnexpectedContext` (CA-5). | Low |

### 1.6 Elevation of privilege

| ID | Threat | Likelihood | Impact | Mitigation | Residual |
|---|---|---|---|---|---|
| CA-E-1 | Provider escalates to admin (e.g. by tricking the admin into signing an `add_provider` for the provider's own future admin address, then `set_admin`). | Low | Critical | Only the admin can call `set_admin`; admin-sep enforces this. A provider has no path to admin promotion. | Low |
| CA-E-2 | UTXO owner (P256) escalates to provider. | Low | Critical | P256 signers are verified per-UTXO; their authority is bound to specific spends. They cannot satisfy the provider check (CA-2) without an Ed25519 signature from a registered provider. | Low |
| CA-E-3 | Compromised admin key escalates further (e.g., to the admin of *other* Channel Auth instances). | n/a | n/a | This is not an in-contract escalation; admin-key compromise is a key-management issue. The Moonlight Security Council multi-sig (Stellar-native) is the documented mitigation (per README's mermaid: "admin = Moonlight Security Council multi-sig"). | n/a (out of contract) |
| CA-E-4 | Stale upgrade WASM (admin-approved historical upgrade) is replayed to revert to a vulnerable version. | Low | Critical | Soroban auth-entry nonces and `valid_until_ledger` prevent literal replay of historical admin signatures. A new upgrade requires a fresh admin signature. | Low |

---

## 2. Privacy Channel

### 2.1 Spoofing

| ID | Threat | Likelihood | Impact | Mitigation | Residual |
|---|---|---|---|---|---|
| PC-S-1 | Attacker submits a `transact` claiming to be a legitimate UTXO owner. | High | Critical | Every spend in `bundle.spend` requires a P256 signature in `signatures` whose pubkey matches the spend's UTXO key. Verification chain: `process_bundle` → `auth.require_auth_for_args` → Channel Auth's `__check_auth` → `handle_utxo_auth`. (CA-4, PC-13/14) | Low |
| PC-S-2 | Attacker spoofs a depositor (forces another G-account's balance into a channel). | Medium | High | `execute_external_operations` calls `from.require_auth_for_args(vec![&e, conditions])` for each deposit (PC-13). The depositor must explicitly authorize the exact condition list. | Low |
| PC-S-3 | Attacker spoofs the asset contract by deploying a malicious SAC at a guessed address and then somehow registering it as the channel's asset. | Low | Critical | The asset is set once at construction (PC-1) and has no externally callable mutator. Constructor-time selection is a deployment-policy concern handled off-chain. | Low |
| PC-S-4 | Attacker spoofs the auth contract. | Low | Critical | Same as PC-2 (immutable auth binding). | Low |

### 2.2 Tampering

| ID | Threat | Likelihood | Impact | Mitigation | Residual |
|---|---|---|---|---|---|
| PC-T-1 | Attacker tampers with a bundle to spend a UTXO they don't own. | High | Critical | P256 verification per spend (PC-S-1 / CA-4). Tampering with the spend list invalidates the signature payload. | Low |
| PC-T-2 | Attacker tampers with a `Create` amount to inflate UTXO balances out of thin air. | High | Critical | `process_bundle` enforces `total_available_balance == expected_outgoing` (PC-4). Inflated creates exceed available balance and panic with `UnbalancedBundle`. | Low |
| PC-T-3 | Attacker tampers with `op.deposit` amounts to drain a depositor without their auth. | Medium | High | Depositor's `require_auth_for_args(conditions)` only authorizes the *condition* list, not the amount directly — but the conditions reference the UTXO outcomes that determine what the deposit funds. The amount in `(addr, amount, conditions)` is what the SAC `transfer(from, channel, amount)` uses; the condition's `Create(utxo, amount)` references the matching create amount. If the amounts diverge, the bundle balance equation (PC-4) panics. **There is, however, a subtle case**: a deposit `(from, amount, [])` with empty conditions has no condition coverage but still requires `from`'s auth-for-args of an empty `vec![]`; the depositor would need to sign an explicitly-empty condition list, which the SDK would not normally produce. Worth the auditor's attention. | **Medium** |
| PC-T-4 | Attacker tampers with `op.withdraw` recipient to redirect funds. | High | Critical | Withdrawal authorization comes from the *spend* signatures: every `Spend` condition lists the expected `ExtWithdraw(to, amount)` as a condition. Tampering with the recipient changes the conditions, which changes the auth payload, which invalidates the signature. (CA-4, PC-14) | Low |
| PC-T-5 | Storage tampering — attacker mutates UTXO state directly. | Low | Critical | Soroban storage is contract-scoped; no public mutator. Persistent storage entries are authenticated by Soroban's storage protocol. | Low |
| PC-T-6 | Attacker manipulates `Supply` directly. | Low | Critical | `read_supply` / `write_supply_unchecked` are not exposed externally. The only writers are `increase_supply` / `decrease_supply` in `treasury.rs`, called only from `execute_external_operations`. (PC-3) | Low |

### 2.3 Repudiation

| ID | Threat | Likelihood | Impact | Mitigation | Residual |
|---|---|---|---|---|---|
| PC-R-1 | Channel operator denies that a particular `transact` happened. | Low | Medium | The transaction is on the public ledger; storage-state delta and gas cost are non-repudiable. | Low |
| PC-R-2 | UTXO owner denies having spent / created a particular UTXO. | Low | Medium | P256 signatures are part of the auth-entry XDR and the source of truth. | Low |
| PC-R-3 | Channel-internal-only `transact` (Spend+Create only, no Ext*) leaves no contract-level event. | Certain | Medium | **Documented gap** (arch §3.4, static-analysis F-MANUAL-6). Privacy Channel is configured with `no-utxo-events` and `no-bundle-events`. The transaction is observable in Stellar metadata but no per-bundle Soroban event is emitted. This is a deliberate design choice for cost optimization and aligns with the privacy goals of the channel; however, it means downstream auditors and indexers must read transaction footprints rather than subscribing to events. | **Medium** |
| PC-R-4 | Depositor denies authorizing a deposit. | Low | Low | Depositor's Ed25519 auth-entry signature is captured in transaction metadata. | Low |

### 2.4 Information disclosure

| ID | Threat | Likelihood | Impact | Mitigation | Residual |
|---|---|---|---|---|---|
| PC-I-1 | Channel `Supply` is publicly readable. | Certain (by design) | None — public by design | The aggregate channel size is a public protocol fact. | None |
| PC-I-2 | Per-UTXO balance is queryable via `utxo_balance(utxo_pk)`. | Certain (by design) | Low | If an observer knows a specific UTXO public key, they can query its balance. The privacy property of the channel relies on UTXO public keys not being correlated with Stellar accounts (the moonlight-sdk derives them via HKDF from a master seed; correlation requires the seed). The contract layer does not enforce this; it is a property of the SDK. | Low |
| PC-I-3 | The set of UTXOs that exist is enumerable by an indexer that watches storage entries. | Certain | Medium — by design | Anyone watching ledger state can enumerate UTXO storage entries. Linking those to identities still requires the SDK-derived seed; contract-layer privacy is "unlinkability via hashed P256 keys", not "non-existence of UTXO records". | Medium — accepted |
| PC-I-4 | Bundle conditions (`Vec<Condition>`) appear in the `AuthRequirements` argument and are visible on-chain to anyone reading the auth-entry XDR. | Certain | Medium — by design | Conditions describe the outcomes a UTXO owner authorizes. They are by design public (the auth flow needs to verify against them). They reveal what UTXO movements happened in a bundle but not who owns the addresses. | Medium — accepted |

### 2.5 Denial of service

| ID | Threat | Likelihood | Impact | Mitigation | Residual |
|---|---|---|---|---|---|
| PC-D-1 | Attacker submits a `transact` with a maximally large `bundle.spend` / `create` to exhaust instruction limits. | Medium | Medium | Soroban metering bounds per-tx instruction count; the attacker pays the resource fee. The bundle-balance loop is linear in (spend + create) size. | Low |
| PC-D-2 | Attacker repeatedly submits failing bundles to grief the provider. | Medium | Low | Each failed submission costs the *submitter* the resource fee. The provider's only exposure is the cost of producing an Ed25519 signature, which is negligible. The provider platform's mempool layer does its own rate-limiting; the contract has no role here. | Low |
| PC-D-3 | Drawer-storage rotation panic on `current_drawer.checked_add(1).expect("drawer overflow")` in `alloc_slot_and_rotate_if_needed_cached`. | Low | Low | Triggered only after `u32::MAX` drawer rotations × 1024 slots ≈ 4.4 trillion UTXO creates. Practically unreachable. | Low |
| PC-D-4 | Attacker submits bundle that creates a UTXO that already exists, forcing a panic and tying up a provider's resource for nothing. | Medium | Low | The bundle is rejected via `UTXOAlreadyExists`. The submitter pays the fee. | Low |
| PC-D-5 | Attacker submits a bundle whose totals overflow `i128`. | Low | Low | `pre_process_channel_operation` uses `checked_add` and panics with `AmountOverflow` on overflow (PC-12). | Low |
| PC-D-6 | Asset SAC reverts during `transfer` (e.g., depositor balance insufficient). | Medium | Low | The whole `transact` reverts atomically (PC-15). Submitter pays gas. | Low |
| PC-D-7 | Channel Auth `__check_auth` itself reverts (e.g., admin disabled all providers, leaving none). | Low | Medium | Admin can `remove_provider` all providers, after which no transact is possible until `add_provider` re-runs. The channel is recoverable but temporarily unusable; this is a governance choice, not a vulnerability. | Low |

### 2.6 Elevation of privilege

| ID | Threat | Likelihood | Impact | Mitigation | Residual |
|---|---|---|---|---|---|
| PC-E-1 | UTXO owner attempts to bypass the channel's bundle-balance equation to mint UTXOs from nothing. | High | Critical | `process_bundle` enforces `total_available_balance == expected_outgoing` (PC-4). | Low |
| PC-E-2 | UTXO owner attempts to bypass deposit auth and pull external funds into a channel. | Medium | High | `from.require_auth_for_args(vec![&e, conditions])` requires the depositor's Ed25519 signature on the exact condition list (PC-13). | Low |
| PC-E-3 | Withdraw recipient claims unauthorized funds. | Low | High | The recipient does not sign; they are named in the spend conditions of an authorized bundle. An attacker would need to be either (a) a UTXO owner already authorizing a withdrawal, or (b) able to forge a bundle's auth — both of which fall back to the auth model. | Low |
| PC-E-4 | Privacy Channel admin escalates to dictate user funds. | Low | High | Admin's only powers are `set_admin` and `upgrade`. Neither directly moves user funds. However, an upgrade to a malicious WASM can grant the admin arbitrary power — so admin compromise is effectively a critical-severity event. The mitigation lives at the key-management layer (Stellar multi-sig for the admin address). | **Medium** |
| PC-E-5 | Channel Auth admin (which may differ from Privacy Channel admin) leverages provider-set control to author bundles. | Medium | Critical | A Channel Auth admin can register their own address as a provider and self-authorize bundles. **However**, providers cannot mint UTXOs without P256 signatures; they can only authorize bundles whose UTXO owners have already signed. So the admin gains the ability to *batch* user-signed operations but not to fabricate them. Net: this is the documented "trusted provider" role; users trust the provider to relay signed operations honestly. | Medium — accepted (architectural) |
| PC-E-6 | Cross-channel privilege escalation (e.g., a UTXO from channel A used in channel B). | Low | High | Auth payload is bound to `caller_contract_address_bytes` (CA-4). A signature for a spend in channel A's contract address is invalid against channel B's contract. | Low |

---

## 3. Cross-contract / system-level threats

### 3.1 Channel Auth ↔ Privacy Channel composition

| ID | Threat | Likelihood | Impact | Mitigation | Residual |
|---|---|---|---|---|---|
| X-1 | A new Privacy Channel is constructed pointing at a malicious Channel Auth (e.g., one whose admin is an attacker). | Low | High | Constructor binding; deployment is a governance action. The Council Console / off-chain process is responsible for vetting which Channel Auth a new channel binds to. Contract-level mitigation: the asset and auth bindings are immutable post-construction (PC-1, PC-2). | Low — relies on deployment hygiene |
| X-2 | Multiple Privacy Channels share a Channel Auth; compromise of that Channel Auth's admin compromises all bound channels. | Low | Critical (if it happens) | Architectural: documented in arch §1 and §2.6. Mitigation lives at the admin key-management layer (multi-sig). | **Medium** — architectural |
| X-3 | A future contract upgrade introduces a subtle invariant break (e.g., adds an `set_asset` mutator). | Low | Critical | All upgrades require admin auth (CA-1). The mitigation is procedural: every upgrade WASM must pass the same audit / review process. The contract layer cannot unilaterally prevent self-authored mistakes. | Medium — procedural |

---

## 4. Residual-risk summary

| Category | Channel Auth | Privacy Channel | Cross-contract |
|---|---|---|---|
| Spoofing | Low (5/5 mitigated) | Low (4/4 mitigated) | n/a |
| Tampering | Low (5/5 mitigated) | Low (5/6 mitigated, 1 medium: PC-T-3 empty-conditions deposit) | n/a |
| Repudiation | Medium (CA-R-2/3 — no events on `set_admin`/`upgrade`) | Medium (PC-R-3 — no events on internal-only transacts) | n/a |
| Information disclosure | None (all by design) | Medium (PC-I-3/4 — accepted by design) | n/a |
| Denial of service | Low | Low | n/a |
| Elevation of privilege | Low (4/4 mitigated) | Medium (PC-E-4 admin upgrade, PC-E-5 admin-can-be-provider) | Medium (X-2 shared admin, X-3 upgrade governance) |

### 4.1 Items the Audit Bank readiness reviewer should especially look at

The team flags the following as the spots most worth the assigned audit firm's attention. None is a known defect; all are areas where a deeper second-opinion would add the most value:

1. **PC-T-3 (deposit with empty conditions list).** The auth payload for the depositor's `require_auth_for_args` is `vec![&e, conditions.into_val(&e)]`. If a client constructs a deposit with an empty conditions list, the depositor must sign over an explicitly-empty list. Whether this is a legitimate path depends on whether the SDK ever produces zero-condition deposits and whether such a payload is acceptable to the underlying SAC. Worth tracing.
2. **PC-R-3 (no events on internal-only transacts).** This is intentional cost optimization but means the on-chain audit trail is shallower than auditors of typical smart contracts may expect.
3. **CA-R-2/3 (no `set_admin` / `upgrade` events).** Combined with the inherited admin-sep traits, this means the audit trail for governance is partially implicit.
4. **F-MANUAL-1 (signature-verifier wrappers).** Not currently exploitable but worth re-validating against any future Soroban SDK upgrade.
5. **F-MANUAL-2 (zero-arg context bypass).** Code-structural rather than vulnerability; coupling that future refactors could break.
6. **X-2 (shared admin compromise).** Architectural rather than code-level. The mitigation lives in the admin key's multi-sig configuration, which is off-contract.

There are **no High residual risks** identified in this STRIDE pass that the form should flag as critical-severity findings to the audit firm. The Medium residuals (PC-R-3, CA-R-2/3, PC-E-4/5, X-2) are documented design choices or architectural realities; we describe them honestly here so that the audit firm can decide whether to dig deeper rather than discovering them mid-engagement.
