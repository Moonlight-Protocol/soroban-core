use moonlight_primitives::{
    equal_condition_sequence, no_duplicate_addresses, verify_condition_does_not_conflict_with_set,
    Condition,
};
use moonlight_utxo_core::core::{calculate_auth_requirements, InternalBundle};
use soroban_sdk::{
    auth::{ContractContext, InvokerContractAuthEntry, SubContractInvocation},
    contracterror, contracttype, panic_with_error,
    token::TokenClient,
    vec, xdr, Address, BytesN, Env, IntoVal, Symbol, Val, Vec,
};

use crate::{
    storage::read_asset,
    treasury::{decrease_supply, increase_supply},
};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    RepeatedAccountForDeposit = 101,
    RepeatedAccountForWithdraw = 102,
    ConflictingConditionsForAccount = 103,
    AmountOverflow = 104,
}

#[derive(Clone)]
#[contracttype]
pub struct ChannelOperation {
    pub spend: Vec<(BytesN<65>, Vec<Condition>)>,
    pub create: Vec<(BytesN<65>, i128)>,
    pub deposit: Vec<(Address, i128, Vec<Condition>)>,
    pub withdraw: Vec<(Address, i128, Vec<Condition>)>,
}

pub fn pre_process_channel_operation(
    e: &Env,
    op: ChannelOperation,
) -> (InternalBundle, i128, i128) {
    let mut total_deposit: i128 = 0;
    for (_addr, amt, _conds) in op.deposit.iter() {
        total_deposit = match total_deposit.checked_add(amt) {
            Some(v) => v,
            None => panic_with_error!(&e, Error::AmountOverflow),
        };
    }

    let mut total_withdraw: i128 = 0;
    for (_addr, amt, _conds) in op.withdraw.iter() {
        total_withdraw = match total_withdraw.checked_add(amt) {
            Some(v) => v,
            None => panic_with_error!(&e, Error::AmountOverflow),
        };
    }

    let merged_deposit_and_withdraws =
        join_external_operations(&e, op.deposit.clone(), op.withdraw.clone());

    let auth_req = calculate_auth_requirements(e, &op.spend); // &vec![&e]);

    //get the spend array without conditions
    let mut spend: Vec<BytesN<65>> = Vec::new(&e);
    for (spend_utxo, _conditions) in op.spend.iter() {
        spend.push_back(spend_utxo.clone());
    }

    let utxo_op = InternalBundle {
        spend: spend,
        create: op.create,
        req: auth_req,
    };

    let mut condition_lists: Vec<Vec<Condition>> = Vec::new(&e);
    for (_, conds) in op.spend.iter() {
        condition_lists.push_back(conds.clone());
    }
    for (_, _, conds) in op.deposit.iter() {
        condition_lists.push_back(conds.clone());
    }
    for (_, _, conds) in op.withdraw.iter() {
        condition_lists.push_back(conds.clone());
    }

    // TODO: REVIEW CONDITIONS AGAINST RESULTS
    let _external_condition_list: Vec<Condition> =
        extract_external_condition_lists(e, condition_lists);

    return (utxo_op, total_deposit, total_withdraw);
}

// Joins deposit and withdraw operations into a single Vec<(Address, Vec<Condition>)>.
fn join_external_operations(
    e: &Env,
    deposit: Vec<(Address, i128, Vec<Condition>)>,
    withdraw: Vec<(Address, i128, Vec<Condition>)>,
) -> Vec<(Address, Vec<Condition>)> {
    verify_external_operations(&e, deposit.clone(), withdraw.clone());

    let mut combined: Vec<(Address, Vec<Condition>)> = Vec::new(&e);
    for (addr, _, conditions) in deposit {
        combined.push_back((addr, conditions));
    }
    for (addr, _, conditions) in withdraw {
        combined.push_back((addr, conditions));
    }
    combined
}

fn extract_external_condition_lists(e: &Env, list: Vec<Vec<Condition>>) -> Vec<Condition> {
    let mut merged = Vec::new(&e);
    for conditions in list {
        for cond in conditions {
            if !merged.contains(&cond) {
                merged.push_back(cond);
            }
        }
    }
    merged
}

fn verify_external_operations(
    e: &Env,
    deposit: Vec<(Address, i128, Vec<Condition>)>,
    withdraw: Vec<(Address, i128, Vec<Condition>)>,
) {
    if !no_duplicate_addresses(&e, deposit.iter(), |(addr, _amount, _conditions)| {
        addr.clone()
    }) {
        panic_with_error!(&e, Error::RepeatedAccountForDeposit);
    }
    if !no_duplicate_addresses(&e, withdraw.iter(), |(addr, _amount, _conditions)| {
        addr.clone()
    }) {
        panic_with_error!(&e, Error::RepeatedAccountForWithdraw);
    }

    // If an address is both depositing and withdrawing, the condition sequences must be identical (order + content).
    for (dep_addr, _, dep_conds) in deposit.iter() {
        for (with_addr, _amt, with_conds) in withdraw.iter() {
            if dep_addr == with_addr {
                if !equal_condition_sequence(&e, &dep_conds, &with_conds) {
                    panic_with_error!(&e, Error::ConflictingConditionsForAccount);
                }
            }
        }
    }
}

pub fn execute_external_operations(
    e: &Env,
    deposit: Vec<(Address, i128, Vec<Condition>)>,
    withdraw: Vec<(Address, i128, Vec<Condition>)>,
) {
    let asset = read_asset(e);

    let asset_client = TokenClient::new(e, &asset);

    for (from, amount, deposit_conditions) in deposit.iter() {
        from.require_auth_for_args(vec![&e, deposit_conditions.into_val(e)]);
        asset_client.transfer(&from, &e.current_contract_address(), &amount);
        increase_supply(&e, amount);
    }

    for (to, amount, _) in withdraw.iter() {
        let args_val: Vec<Val> = vec![
            e,
            (&e.current_contract_address()).into_val(e),
            (&to).into_val(e),
            (&amount).into_val(e),
        ];

        e.authorize_as_current_contract(vec![
            &e,
            InvokerContractAuthEntry::Contract(SubContractInvocation {
                context: ContractContext {
                    contract: asset.clone(),
                    fn_name: Symbol::new(e, "transfer"),
                    args: args_val.clone(),
                },
                sub_invocations: vec![e],
            }),
        ]);
        asset_client.transfer(&e.current_contract_address(), &to, &amount);
        decrease_supply(&e, amount);
    }
}
