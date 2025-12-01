use soroban_sdk::{contracttype, Address, Env};

#[derive(Clone)]
#[contracttype]
pub enum PrivacyChannelDataKey {
    Asset,  //Address
    Supply, //i128
}

pub fn write_asset_unchecked(e: &Env, asset: Address) {
    e.storage()
        .instance()
        .set(&PrivacyChannelDataKey::Asset, &asset);
}

pub fn read_asset(e: &Env) -> Address {
    e.storage()
        .instance()
        .get(&PrivacyChannelDataKey::Asset)
        .unwrap()
}

pub fn write_supply_unchecked(e: &Env, supply: i128) {
    e.storage()
        .instance()
        .set(&PrivacyChannelDataKey::Supply, &supply);
}

pub fn read_supply(e: &Env) -> i128 {
    e.storage()
        .instance()
        .get(&PrivacyChannelDataKey::Supply)
        .unwrap_or(0)
}
