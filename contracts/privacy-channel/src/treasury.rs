use moonlight_errors::Error;
use soroban_sdk::{panic_with_error, Env};

use crate::storage::{read_supply, write_supply_unchecked};

pub fn increase_supply(e: &Env, amount: i128) {
    let supply = read_supply(e);
    match supply.checked_add(amount) {
        Some(new_supply) => {
            write_supply_unchecked(e, new_supply);
        }
        None => panic_with_error!(e, Error::AmountOverflow),
    }
}

pub fn decrease_supply(e: &Env, amount: i128) {
    let supply = read_supply(e);
    match supply.checked_sub(amount) {
        Some(new_supply) => {
            write_supply_unchecked(e, new_supply);
        }
        None => panic_with_error!(e, Error::AmountUnderflow),
    }
}
