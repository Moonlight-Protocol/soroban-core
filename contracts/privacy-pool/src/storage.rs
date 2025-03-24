
use soroban_sdk::{contracttype, Address, Env};



#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Asset,  //Address
}


pub fn write_asset(e: Env, asset: Address) {
    e.storage().instance().set(&DataKey::Asset, &asset);
}

pub fn read_asset(e: &Env) -> Address {
    e.storage().instance().get(&DataKey::Asset).unwrap()
}


pub fn is_contract_initialized(e: &Env) -> bool {   
    e.storage().instance().has(&DataKey::Asset)
}
