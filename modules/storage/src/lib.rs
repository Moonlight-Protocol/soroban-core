#![no_std]

// pick exactly one
#[cfg(all(feature = "storage-simple", feature = "storage-drawer"))]
compile_error!("Enable only one of storage-simple or storage-drawer");
#[cfg(not(any(feature = "storage-simple", feature = "storage-drawer")))]
compile_error!("Enable one of storage-simple or storage-drawer");

// submodules
#[cfg(feature = "storage-drawer")]
mod drawer;
#[cfg(feature = "storage-simple")]
mod simple;

// re exports, single name
#[cfg(feature = "storage-simple")]
pub type Store = simple::SimpleStore;
#[cfg(feature = "storage-drawer")]
pub type Store = drawer::DrawerStore;

#[cfg(feature = "storage-drawer")]
pub use drawer::{DrawerCache, DrawerStore};

#[cfg(feature = "storage-drawer")]
pub const IS_DRAWER: bool = true;
#[cfg(feature = "storage-simple")]
pub const IS_DRAWER: bool = false;

// storage.rs
use soroban_sdk::{contracterror, contracttype};
use soroban_sdk::{Bytes, BytesN, Env};

#[derive(Clone)]
#[contracttype]
pub enum UtxoState {
    Unspent(i128),
    Spent,
}

#[derive(Clone)]
#[contracttype]
pub enum UTXOCoreDataKey {
    UTXO(BytesN<32>),
    // Drawer entries live under DrawerKey below for the optimized impl
}

#[derive(Clone)]
#[contracttype]
pub struct DrawerKey {
    pub id: u32, // sequential drawer id
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    UTXOAlreadyExists = 1,
    UTXODoesntExist = 2,
    UTXOAlreadySpent = 3,
    UnbalancedBundle = 4,
    InvalidCreateAmount = 5,
    RepeatedCreateUTXO = 6,
    RepeatedSpendUTXO = 7,
    UTXONotFound = 8,
}

pub trait UtxoStore {
    fn utxo_balance(e: &Env, utxo65: &BytesN<65>) -> i128;
    fn create(e: &Env, utxo65: &BytesN<65>, amount: i128);
    fn spend(e: &Env, utxo65: &BytesN<65>) -> i128;
    fn hash_utxo_key(e: &Env, utxo65: &BytesN<65>) -> BytesN<32> {
        let b = Bytes::from_slice(e, utxo65.to_array().as_ref());
        let h = e.crypto().sha256(&b);
        BytesN::<32>::from_array(e, &h.to_array())
    }
}
