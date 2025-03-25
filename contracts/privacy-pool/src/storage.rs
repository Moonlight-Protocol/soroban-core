use soroban_sdk::{contracttype, Address, Env};

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Asset,                    //Address
    Admin,                    //Address
    Supply,                   //i128
    ProviderBalance(Address), //i128
}

pub fn write_asset_unchecked(e: &Env, asset: Address) {
    e.storage().instance().set(&DataKey::Asset, &asset);
}

pub fn read_asset(e: &Env) -> Address {
    e.storage().instance().get(&DataKey::Asset).unwrap()
}

pub fn is_contract_initialized(e: &Env) -> bool {
    e.storage().instance().has(&DataKey::Asset)
}

pub fn write_admin_unchecked(e: &Env, admin: Address) {
    e.storage().instance().set(&DataKey::Admin, &admin);
}

pub fn read_admin(e: &Env) -> Address {
    e.storage().instance().get(&DataKey::Admin).unwrap()
}

pub fn write_supply_unchecked(e: &Env, supply: i128) {
    e.storage().instance().set(&DataKey::Supply, &supply);
}

pub fn read_supply(e: &Env) -> i128 {
    e.storage().instance().get(&DataKey::Supply).unwrap_or(0)
}

pub fn write_provider_balance_unchecked(e: &Env, provider: Address, balance: i128) {
    e.storage()
        .instance()
        .set(&DataKey::ProviderBalance(provider), &balance);
}

pub fn read_provider_balance(e: &Env, provider: Address) -> i128 {
    e.storage()
        .instance()
        .get(&DataKey::ProviderBalance(provider))
        .unwrap_or(0)
}
