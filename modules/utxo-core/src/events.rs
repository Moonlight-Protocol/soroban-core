// MOON-10: import the shared names once across either feature so enabling events does not produce
// duplicate-import compile errors. `Vec` is only needed by the bundle event.
#[cfg(any(
    not(feature = "no-bundle-events"),
    not(feature = "no-utxo-events")
))]
use soroban_sdk::{contractevent, BytesN, Symbol};

#[cfg(not(feature = "no-bundle-events"))]
use soroban_sdk::Vec;

#[cfg(not(feature = "no-bundle-events"))]
#[contractevent(data_format = "vec")]
pub struct BundleEvent {
    #[topic]
    pub name: Symbol, // bundle
    pub spend: Vec<BytesN<65>>,
    pub create: Vec<(BytesN<65>, i128)>,
    pub deposited: i128,
    pub withdrawn: i128,
}

#[cfg(not(feature = "no-utxo-events"))]
#[contractevent(data_format = "vec")]
pub struct UtxoEvent {
    #[topic]
    pub name: Symbol, // "utxo"
    pub utxo: BytesN<65>,
    pub action: Symbol,
    pub amount: i128,
}
