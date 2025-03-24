# Provider Management Module

This module manages the privacy providers for a privacy pool in Soroban smart contracts. It allows contracts to register, deregister, and verify providers by their address. Providers are used to handle specific operations within the privacy pool.

(WIP): Balance management

## Core Capabilities

- **Provider Verification:**  
  Check if a given address is registered as a provider.

- **Provider Registration:**  
  Register a new provider, ensuring that each provider is added only once.

- **Provider Deregistration:**  
  Remove a provider from the registry.

- **Authorization Enforcement:**  
  Require that a provider is registered and that the transaction is authorized by that provider, ensuring secure access control.

## Integration

To use this module in your contract, add it as a dependency in your `Cargo.toml`:

```toml
[dependencies]
provider-management = { path = "path/to/provider-management" }
```

Then, import and call the provided functions to manage provider access within your privacy pool.
