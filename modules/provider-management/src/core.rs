use soroban_sdk::{contracttype, Address, Env};

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Provider(Address),
}

/// Provider management trait.
///
/// This trait provides functions for managing registered providers.
///
pub trait ProviderManagementTrait {
    fn register_provider(e: Env, provider: Address);
    fn deregister_provider(e: Env, provider: Address);
    fn is_provider(e: Env, provider: Address) -> bool;
}

/// Checks if the given address is a registered provider.
///
/// Returns `true` if the provider is registered, `false` otherwise.
///
pub fn is_provider(e: &Env, provider: Address) -> bool {
    e.storage()
        .instance()
        .get::<_, ()>(&DataKey::Provider(provider))
        .is_some()
}

/// Registers a new provider.
///
/// ### Panics
/// - Panics if the provider is already registered.
pub fn register_provider(e: &Env, provider: Address) {
    assert!(
        !is_provider(&e, provider.clone()),
        "Provider already registered"
    );

    e.storage()
        .instance()
        .set(&DataKey::Provider(provider), &());
}

/// Deregisters a provider.
///
/// ### Panics
/// - Panics if the provider is not registered.
pub fn deregister_provider(e: &Env, provider: Address) {
    assert!(is_provider(&e, provider.clone()), "Provider not registered");

    e.storage().instance().remove(&DataKey::Provider(provider));
}

/// Requires that the given provider is registered
///  and that the transaction is authorized by the provider.
///
/// ### Panics
/// - Panics if the provider is not registered.
/// - Panics if the transaction is not authorized by the provider.
pub fn require_provider(e: &Env, provider: Address) {
    assert!(is_provider(&e, provider.clone()), "Provider not registered");

    provider.require_auth();
}
