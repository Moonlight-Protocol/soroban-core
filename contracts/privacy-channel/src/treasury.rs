use soroban_sdk::{Address, Env};

use crate::storage::{read_supply, write_supply_unchecked};

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
