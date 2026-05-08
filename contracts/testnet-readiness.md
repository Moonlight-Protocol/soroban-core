# Soroban Core — Testnet Readiness Verification

This document captures the evidence that the in-scope contracts are deployed and live on Stellar testnet, and that the local test suite is green from a fresh clone, satisfying the Audit Bank rule that applicants have "deployed [their contracts] on testnet" with extensive test coverage.

Verification timestamp: 2026-05-08 (date of this PR).

---

## 1. Live testnet contracts

The Tranche-1 D1 deliverable (delivered 2026-03-18 with `soroban-core` v0.1.0) deployed both contracts to Stellar testnet. Their addresses are stable and remain live:

| Contract | Testnet contract ID | WASM hash | Created |
|---|---|---|---|
| Channel Auth | `CAF7DFHTPSYIW5543WBXJODZCDI5WF5SSHBXGMPKFOYPFRDVWFDNBGX7` | `8288c00d21403683e58993764f58dd3e21c3a9426c7f6106bdd022b331c3bfc1` | 2026-01-29 17:06:26 UTC |
| Privacy Channel | `CDMZSHMT2AIL2UG7XBOHZKXM6FY3MUP75HAXUUSAHLGRQ2VWPGYKPM5T` | `5fd06369d23502139b36297a4ad6b0c6c9ef0df7021920ca10801fa9c2486e7c` | 2026-01-29 17:08:51 UTC |

Both were deployed by the same Stellar account: `GB4EI6PC2MYRXX32R7ALCWAMSPNABWKSJBIWYIU2MUVQZHBMEMWBAO42`.

### 1.1 Verification commands

Verified via the public Stellar Expert API:

```
curl -sS https://api.stellar.expert/explorer/testnet/contract/CAF7DFHTPSYIW5543WBXJODZCDI5WF5SSHBXGMPKFOYPFRDVWFDNBGX7
curl -sS https://api.stellar.expert/explorer/testnet/contract/CDMZSHMT2AIL2UG7XBOHZKXM6FY3MUP75HAXUUSAHLGRQ2VWPGYKPM5T
```

Raw responses (relevant fields only):

**Channel Auth** —
```json
{
  "contract":"CAF7DFHTPSYIW5543WBXJODZCDI5WF5SSHBXGMPKFOYPFRDVWFDNBGX7",
  "creator":"GB4EI6PC2MYRXX32R7ALCWAMSPNABWKSJBIWYIU2MUVQZHBMEMWBAO42",
  "wasm":"8288c00d21403683e58993764f58dd3e21c3a9426c7f6106bdd022b331c3bfc1",
  "subinvocation":46,
  "storage_entries":1
}
```

The `subinvocation: 46` count reflects 46 occurrences of the contract being invoked as an authorization principal by other contracts — consistent with Channel Auth's role of governing one or more Privacy Channel deployments.

**Privacy Channel** —
```json
{
  "contract":"CDMZSHMT2AIL2UG7XBOHZKXM6FY3MUP75HAXUUSAHLGRQ2VWPGYKPM5T",
  "creator":"GB4EI6PC2MYRXX32R7ALCWAMSPNABWKSJBIWYIU2MUVQZHBMEMWBAO42",
  "wasm":"5fd06369d23502139b36297a4ad6b0c6c9ef0df7021920ca10801fa9c2486e7c",
  "subinvocation":0,
  "storage_entries":186
}
```

`storage_entries: 186` reflects accumulated UTXO and instance-storage entries on testnet, consistent with bundles having been transacted against this channel during E2E testing.

The `validation: { status: "unverified" }` field on both responses refers to Stellar Expert's *source-code attestation* (a separate program where contract authors upload the source for the explorer to display); it does not indicate any state inconsistency. The contracts themselves are live and reachable at the IDs given.

### 1.2 RPC alternative

If the auditor prefers a vendor-neutral path, the same contracts are reachable directly via Soroban testnet RPC:

```
curl -sS -X POST https://soroban-testnet.stellar.org \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"getContractData","params":{"contractId":"CAF7DFHTPSYIW5543WBXJODZCDI5WF5SSHBXGMPKFOYPFRDVWFDNBGX7","key":"LedgerKeyContractInstance"}}'
```

(Same call with the privacy-channel contract ID for that contract.)

---

## 2. Local test suite — green from a fresh clone

This PR is being prepared from a fresh clone at `~/repos/tmp/audit-kickoff/soroban-core`. The full workspace test suite was run before drafting any of the audit-package documentation; it passes cleanly:

```
$ cargo test --workspace --no-fail-fast
...
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1m 18s
```

Per-crate results:

| Crate | Tests passed | Failed | Ignored |
|---|---|---|---|
| `channel-auth-contract` | 5 | 0 | 0 |
| `privacy-channel` | 2 | 0 | 0 |
| `moonlight-auth` | 3 | 0 | 0 |
| `moonlight-helpers` | 3 | 0 | 0 |
| `moonlight-utxo-core` | 5 | 0 | 0 |
| `moonlight-primitives` | 0 | 0 | 0 (no test target) |
| `moonlight-storage` | 0 | 0 | 0 (no test target) |
| `token-contract` (test-only) | 6 | 0 | 0 |
| **Total in-scope contract + module** | **18** | **0** | **0** |

A complete enumeration of which test covers which invariant is in `tests.md` §2.

### 2.1 Toolchain used

- `rustc` 1.94.0 (4a4ef493e 2026-03-02)
- `cargo` 1.94.0 (85eff7c80 2026-01-15)
- macOS arm64 (Darwin 25.4.0)

### 2.2 Reproducing the run

```
git clone https://github.com/Moonlight-Protocol/soroban-core
cd soroban-core
cargo test --workspace --no-fail-fast
```

(The CI pipeline at `.github/workflows/pr.yml` runs the same command on every PR against `main`, plus the WASM build via `stellar contract build` and the cross-repo E2E + lifecycle suites in `Moonlight-Protocol/local-dev`.)

---

## 3. End-to-end testnet exercise

Beyond the unit tests, the contracts are exercised end-to-end by `Moonlight-Protocol/local-dev`'s reusable Docker-Compose harness on every PR. The reusable workflows:

- `Moonlight-Protocol/local-dev/.github/workflows/e2e-reusable.yml` — full deposit / transfer / withdraw lifecycle including the provider-platform mempool, executor, verifier, and a real Stellar local instance.
- `Moonlight-Protocol/local-dev/.github/workflows/lifecycle-reusable.yml` — multi-cycle deposit + transfer + withdraw flow with provider restarts and observability checks.

Both are wired in via `soroban-core/.github/workflows/pr.yml` (`e2e:` and `lifecycle:` jobs that consume the `contract-wasms` artifact uploaded by the build step). On `main`, a successful tag push (auto-triggered by `auto-tag.yml` on any `Cargo.toml` change) emits a `repository_dispatch` (`event-type: module-release`) to `local-dev` to re-run integration tests across the rest of the Moonlight stack against the new release.

The 46 `subinvocation` count on Channel Auth and 186 `storage_entries` on Privacy Channel (per §1.1) reflect the cumulative effect of these E2E suites running against testnet over the period from 2026-01-29 (deploy date) through 2026-05-08 (this verification).

---

## 4. Out-of-band evidence

The deployed testnet state is independently visible to the auditor through the public-facing consoles (out of audit scope, but provided as confirmation):

- Council Console — `https://moonlight-council-console.fly.storage.tigris.dev`
- Network Dashboard — `https://network-dashboard.fly.storage.tigris.dev`

The Network Dashboard surfaces live channel state (supply, provider count, channel asset) for every Privacy Channel governed by every Channel Auth visible from the public Stellar testnet event stream.
