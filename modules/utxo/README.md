# UTXO Module for Soroban

This module provides a robust UTXO-based system for managing UTXO creation, transfers, and consumption within Soroban smart contracts. It enables contracts to manage UTXOs, where each UTXO is identified by a public key and holds a specific balance. All transfers, burns, and related operations are secured via ECDSA signature verification over deterministic payloads.

Developers can seamlessly integrate this UTXO system into their Soroban contracts by leveraging the provided functions. The module supports core operations such as minting, burning, and transferring UTXOs, as well as batch processing and delegated transfers. This flexible design enables you to build custom logic on top of basic UTXO management—for example, implementing fee collection mechanisms, enforcing conditional spending rules, or customizing state transitions.

By extending or composing these operations, you can tailor your contract’s behavior to meet specific business requirements while ensuring secure and efficient management of UTXO balances.

---

## ✨ Features

- **Minting:**  
  Create new UTXOs with specified balances. Batch operations are supported to efficiently process multiple mints in a single transaction.

- **Burning:**  
  Consume UTXOs using cryptographic signatures. Batch burning is supported, ensuring that each UTXO is paired with its corresponding signature.

- **Transfer Bundles:**  
  Execute atomic multi-input, multi-output transfers by grouping UTXO spend and create operations into bundles. Each bundle is balanced so that the total value spent equals the total value created, enabling efficient execution of complex transfers.

- **Delegated Transfers:**  
  Support delegated transfers where a delegate collects fees from multiple bundle transfers. Unlike standard transfers that require perfect balance, delegated transfers allow for leftover funds to be handled dynamically and assigned to the delegate utxo. The module provides both a standard mode—where any leftover is credited to a delegate UTXO—and a variant that calculates the leftover for further processing using a custom action payload(see below).

- **Custom Payloads for Action-Specific Signature Verification:**  
  Bundle payloads include a custom action string (e.g., "CUSTOM") so that a bundle’s signature is valid only for its intended outcome. This mechanism does not handle any leftover funds automatically; it is the caller's responsibility to process them appropriately. If the leftover funds are not assigned to a UTXO or otherwise handled, they are considered burned.

  This allows for custom features to be implemented on top of a bundle processing such as mobilizing multiple utxos to deposit and aggregated balance in a liquidity pool that doesn't use the utxo model directly.

- **Optional Event Emission:**  
  A custom macro conditionally compiles event emissions based on feature flags. This mechanism allows the contract to suppress internal events—either to optimize costs in high-volume scenarios or to enable custom event handling as needed. See morea the the 'Feature Flags' section.

---

## 🔐 UTXO Security and Signature Framework

**UTXO Representation & Status:**  
 Each UTXO is uniquely identified by a public key and is associated with a balance. A UTXO is considered _unspent_ when it holds a positive balance and _spent_ when its balance is zero. If no record exists for a UTXO (indicated by a balance of –1), it is available for creation. The public key is used for ECDSA signature verification (using secp256r1), ensuring that only authorized operations can modify a UTXO.

**Consistent Payload Derivation for Authorization:**  
 Every UTXO operation relies on an authorization payload that is deterministically derived from data about the UTXO and the specific transaction outcome. This payload serves as a secure summary of the operation details and must be signed by the UTXO owner’s secret key to authorize spending.

The payload combines core data—such as the UTXO identifier and the current transaction parameters—with additional fields (for example, an action string) when needed. This design not only ensures that the signature strictly authorizes the intended operation but also provides extensibility for custom scenarios by allowing extra data to be included as part of the payload.

**Signature Verification & Authorization:**  
 Operations that move funds by spending a utxo require an ECDSA signature over the derived payload. To authorize spending of a UTXO, the payload must be signed by the secret key corresponding to the UTXO's public key. The signature is then verified against the public key, ensuring that only the rightful owner can authorize changes to that UTXO.

**Immutability & Secure State Transition:**  
 Once created, a UTXO can only transition from _unspent_ to _spent_, preserving its transaction history and preventing reuse. Rigorous signature checks and consistent payload derivation guarantee that every UTXO operation occurs exactly as intended by the signer.

---

### ⚙️ Feature Flags

This crate supports compile-time feature flags to control various optional behaviors through conditional compilation. For example, you can disable certain internal macros that emit events or trigger custom behavior, which helps optimize costs or enable custom handling.

| Feature Flag         | Effect                                    | Identifier Used |
| -------------------- | ----------------------------------------- | --------------- |
| `no-utxo-events`     | Disables all UTXO-related event emissions | `"utxo"`        |
| `no-delegate-events` | Disables delegate reward event emissions  | `"delegate"`    |

Event emission is managed via macros such as:

```rust
emit_optional_event!("utxo", env, utxo_key, symbol_short!("spend"), amount);
```

When compiled with, for example, `--features no-utxo-events`, the associated code is omitted.

To use this module in your contract and enable a custom flag like UTXO-related events (or other optional behaviors), add it to your `Cargo.toml` like this:

```toml
[dependencies]
utxo-handler = { path = "../modules/utxo", features = ["no-utxo-events"] }
```

This configuration ensures that any macros controlled by the `no-utxo-events` flag (and later, others) are disabled at compile time.

---

## 🧪 Testing

### Running Tests

- **Run all tests, including flag variants:**

  ```bash
  make test
  ```

---
