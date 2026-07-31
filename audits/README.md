# Audits

## Runtime Verification, 2026-07-23

[Final report (PDF)](runtime-verification-2026-07-23/Moonlight.pdf) ·
[Markdown](runtime-verification-2026-07-23/Moonlight.md)

Runtime Verification Inc. reviewed the Moonlight core contracts over two weeks beginning
2026-06-29, followed by a week of remediation support. Engaged through the SDF Soroban
Audit Bank.

- **Scope commit:** `d65780bbf0a6f8b2b601e4a536b38000775c8a15`
- **Contracts in scope:** `contracts/channel-auth` (Quorum Auth) and
  `contracts/privacy-channel`
- **Final reviewed remediation:** `867c23da1769a2f56e42449ce5d5315ffb5f4cdc`, released as
  [v0.5.0](https://github.com/Moonlight-Protocol/soroban-core/releases/tag/v0.5.0)

Three findings in the A series and twelve informative findings in the B series. A1-A3 and
B1-B11 are resolved; B12 is acknowledged and addressed in documentation.

| Finding | Fix |
|---|---|
| A1 - `hash_payload` encoding is not injective | #38 (with moonlight-sdk#44) |
| A2 - UTXO keys never validated, value under a malformed key locked permanently | #44 |
| A3 - byte-identical signed effects merge into one execution | #45 |
| B1 - `set_admin` grants the maximum acceptance window | #40 |
| B2 - bundle balance accumulator uses unchecked arithmetic | #39 |
| B3 - `hash_payload` doc specifies the wrong integer width | #38 |
| B4 - redundant reentrancy guard | #41 |
| B5 - fragmented and partially duplicated validation across the transact flow | #43 |
| B6-B9 - provider and auth layer informationals | #47 |
| B10, B11 - channel transact layer | #46 |
| B12 - `enable_channel` / `disable_channel` are event-only | #48 |

### Verifying that the audited code is what is deployed

The v0.5.0 release carries the compiled WASM artifacts. Their hashes match the bytecode
deployed on Stellar mainnet, so the audit applies to the running contracts:

```
shasum -a 256 channel_auth_contract.wasm
# 9fbb577757daeb34dcfd597ab4b96b0fadfe6a5f5fdb26320bf6fe6d026c9414
shasum -a 256 privacy_channel.wasm
# b0f3b5aa10f85091f47195cca41e182626ae395c022effad65bbcace3124dd06
```

Compare against a deployed contract's WASM hash via `getLedgerEntries` or on
stellar.expert. Mainnet deployments as of 2026-07-31:

| Contract | Address |
|---|---|
| Channel Auth (Council Salem) | `CABD46PWY4NN7VTXETAUZE5MVS5PRGMWCR2UV74RS25GQR3VXMYTTSEV` |
| Privacy Channel (Council Salem) | `CCLTT2ZJMMSKMUFTMDGZRRT76LFXK6INYM35VFKVZF5ZB4S7LQVEDZZ7` |
| Channel Auth (Council Cheshire) | `CCQPBATB3M3KWHLFGBHDYYTQYVWDK5ZAF2KMJJ2ITO7HU4GSS7X3Y6HY` |
| Privacy Channel (Council Cheshire) | `CBDETE3LWQGQYGFAC3CPAQO2MWNSK4T27U2NBFAIAOWERGQFO7ZDW4LF` |

## Internal reviews

Two internal audits preceded the external engagement and were remediated before it began:
a preemptive self-audit (2026-06-11, findings MOON-01..10, remediated in #32 and released
as 0.2.1) and a second internal review (2026-06-23) that reconciled those findings and
confirmed none had regressed. Reports live in the `pm-theahaco` repository.
