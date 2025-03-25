# Privacy Pool Contract

This Soroban smart contract implements a privacy-preserving asset pool using a UTXO-based accounting model. It allows users to deposit funds into UTXO balances and transact anonymously through the use of cryptographic proofs and signature-based UTXO spending. A permissioned set of privacy providers can be defined and manage to establish a trusted layer for submitting transactions for processing while preserving the privacy of en-users.

For specifics about the utxo model and capabilites refer to the `utxo` module.

---

## 🧩 Key Capabilities

- **Deposits & Withdrawals**  
  Users can deposit tokens into the pool and receive UTXOs that represent private balances. Withdrawals require signed authorization over deterministic payloads to ensure secure and verifiable UTXO spending.

- **UTXO Transfers & Bundles**  
  Supports native UTXO-based transfers, including multi-input, multi-output bundle execution. Both direct and delegated transfers are supported.

- **Delegated Transfers (Providers)**  
  Authorized privacy providers can process transfers on behalf of users and optionally collect remaining balances (change) either as UTXOs or internal balances. This can be used as a mechanism to collect fees for the extended privacy service provided.

WIP: Support to defi applications directly through UTXOs.

---

## 🛡 Security Considerations

- All UTXO operations require valid ECDSA signatures using the secp256r1 curve.
- Provider actions require both registration and explicit authorization.
- Withdrawals validate full UTXO ownership and balance correctness before processing.
