#[cfg(not(feature = "no-bundle-events"))]
use soroban_sdk::{contractevent, BytesN, Symbol, Vec};

#[cfg(not(feature = "no-utxo-events"))]
use soroban_sdk::{contractevent, BytesN, Symbol};

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
