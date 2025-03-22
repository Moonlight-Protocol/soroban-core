/// Conditionally emits an event based on the provided category and feature flags.
///
/// # Overview
/// This macro checks if a specific feature flag is disabled. If so, it emits an event using the provided
/// environment, UTXO (or key), symbol, and amount. If the feature flag is enabled, the event code is omitted,
/// allowing you to optimize operational costs or implement custom behavior.
///
/// # Usage
/// - For UTXO events, use:
/// ```rust
/// use soroban_sdk::{Env, symbol_short, BytesN};
/// use utxo_handler::emit_optional_event;
///
/// let env = Env::default();
/// let key = BytesN::<65>::from_array(&env, &[0u8; 65]);
/// let amount = 100;
/// emit_optional_event!("utxo", env, key, symbol_short!("create"), amount);
/// ```
/// When the `no-utxo-events` feature flag is enabled, the event is not emitted.
///
#[macro_export]
macro_rules! emit_optional_event {
    ("utxo", $e:expr, $utxo:expr, $symbol:expr, $amount:expr) => {
        #[cfg(not(feature = "no-utxo-events"))]
        {
            $e.events().publish(($utxo, $symbol), $amount);
        }
    };
    ("delegate", $e:expr, $utxo:expr, $symbol:expr, $amount:expr) => {
        #[cfg(not(feature = "no-delegate-events"))]
        {
            $e.events().publish(($utxo, $symbol), $amount);
        }
    };
}
