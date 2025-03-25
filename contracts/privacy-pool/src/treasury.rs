use soroban_sdk::{Address, Env};

use crate::storage::{
    read_provider_balance, read_supply, write_provider_balance_unchecked, write_supply_unchecked,
};

pub fn increase_supply(e: &Env, amount: i128) {
    let supply = read_supply(e);
    match supply.checked_add(amount) {
        Some(new_supply) => {
            write_supply_unchecked(e, new_supply);
        }
        None => panic!("Overflow occurred while increasing supply"),
    }
}

pub fn decrease_supply(e: &Env, amount: i128) {
    let supply = read_supply(e);
    match supply.checked_sub(amount) {
        Some(new_supply) => {
            write_supply_unchecked(e, new_supply);
        }
        None => panic!("Underflow occurred while decreasing supply"),
    }
}

pub fn increase_provider_balance(e: &Env, provider: Address, amount: i128) {
    let balance = read_provider_balance(e, provider.clone());
    match balance.checked_add(amount) {
        Some(new_balance) => {
            write_provider_balance_unchecked(e, provider, new_balance);
        }
        None => panic!("Overflow occurred while increasing provider balance"),
    }
}

pub fn decrease_provider_balance(e: &Env, provider: Address, amount: i128) {
    let balance = read_provider_balance(e, provider.clone());
    match balance.checked_sub(amount) {
        Some(new_balance) => {
            write_provider_balance_unchecked(e, provider, new_balance);
        }
        None => panic!("Underflow occurred while decreasing provider balance"),
    }
}
