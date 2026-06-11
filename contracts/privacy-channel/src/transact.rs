use moonlight_errors::Error;
use moonlight_primitives::{
    condition_does_not_conflict_with_set, equal_condition_sequence, no_duplicate_addresses,
    Condition,
};
use moonlight_utxo_core::core::{calculate_auth_requirements, InternalBundle};
use soroban_sdk::{
    assert_with_error,
    auth::{ContractContext, InvokerContractAuthEntry, SubContractInvocation},
    contracttype,
    panic_with_error,
    token::TokenClient,
    vec,
    xdr::ToXdr,
    Address, Bytes, BytesN, Env, IntoVal, Map, Symbol, Val, Vec,
};

use crate::{
    storage::read_asset,
    treasury::{decrease_supply, increase_supply},
};

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
    assert_with_error!(
        &e,
        op_has_no_conflicting_conditions(&e, &op),
        Error::BundleHasConflictingConditions
    );

    let mut total_deposit: i128 = 0;
    for (_addr, amt, _conds) in op.deposit.iter() {
        // MOON-05: reject non-positive amounts in-contract rather than relying on the asset SAC.
        assert_with_error!(&e, amt > 0, Error::InvalidExternalAmount);
        total_deposit = match total_deposit.checked_add(amt) {
            Some(v) => v,
            None => panic_with_error!(&e, Error::AmountOverflow),
        };
    }

    let mut total_withdraw: i128 = 0;
    for (_addr, amt, _conds) in op.withdraw.iter() {
        assert_with_error!(&e, amt > 0, Error::InvalidExternalAmount);
        total_withdraw = match total_withdraw.checked_add(amt) {
            Some(v) => v,
            None => panic_with_error!(&e, Error::AmountOverflow),
        };
    }

    verify_external_operations(&e, op.deposit.clone(), op.withdraw.clone());

    // MOON-01: bind executed effects to owner-signed conditions. The balance check in
    // `process_bundle` only guarantees value conservation, not *where* the value goes; without
    // this, a provider can keep `op.spend` byte-identical (owner P256 sig still verifies) and
    // redirect `op.create` / `op.withdraw` to an attacker while staying balanced. This enforces
    // that the set of executed create/withdraw effects EXACTLY equals the set of Create/ExtWithdraw
    // conditions signed by the spend owners (P256) and depositors (Ed25519).
    assert_executed_effects_are_authorized(&e, &op);

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

    return (utxo_op, total_deposit, total_withdraw);
}

/// MOON-01 binding: enforce that the bundle's executed create/withdraw effects are exactly the
/// set of `Create` / `ExtWithdraw` conditions that were cryptographically authorized.
///
/// Authorized set = every `Condition::Create` / `Condition::ExtWithdraw` found in the spend
/// conditions (P256-signed by the UTXO owner, verified via the auth contract) and the deposit
/// conditions (Ed25519-signed by the depositor via `require_auth_for_args`). Withdraw-tuple
/// conditions are unsigned and are intentionally NOT a source of authorization. `ExtDeposit`
/// (already bound by the depositor's SAC-transfer auth) and `ExtIntegration` (never executed) are
/// not execution-bound and are ignored.
///
/// Executed set = `op.create` rendered as `Condition::Create` and `op.withdraw` rendered as
/// `Condition::ExtWithdraw`.
///
/// Both sets are compared by canonical XDR bytes with set (dedup) semantics, so a multi-spend
/// bundle where each spend repeats (or partitions) the full output set is accepted as long as the
/// union matches the executed effects.
///
/// ### Panics
/// - `UnauthorizedOperation` if an executed effect is not authorized, or an authorized
///   create/withdraw condition is not executed.
fn assert_executed_effects_are_authorized(e: &Env, op: &ChannelOperation) {
    let mut authorized: Map<Bytes, ()> = Map::new(e);
    collect_authorized_effects(e, &mut authorized, &op.spend);
    collect_authorized_effects_from_external(e, &mut authorized, &op.deposit);

    let mut executed: Map<Bytes, ()> = Map::new(e);
    for (utxo, amount) in op.create.iter() {
        executed.set(Condition::Create(utxo, amount).to_xdr(e), ());
    }
    for (addr, amount, _conds) in op.withdraw.iter() {
        executed.set(Condition::ExtWithdraw(addr, amount).to_xdr(e), ());
    }

    // Exact set-equality: |executed| == |authorized| and executed ⊆ authorized ⇒ sets equal.
    assert_with_error!(
        e,
        executed.len() == authorized.len(),
        Error::UnauthorizedOperation
    );
    for key in executed.keys().iter() {
        assert_with_error!(e, authorized.contains_key(key), Error::UnauthorizedOperation);
    }
}

fn collect_authorized_effects(
    e: &Env,
    set: &mut Map<Bytes, ()>,
    spend: &Vec<(BytesN<65>, Vec<Condition>)>,
) {
    for (_utxo, conditions) in spend.iter() {
        for cond in conditions.iter() {
            if is_execution_bound(&cond) {
                set.set(cond.to_xdr(e), ());
            }
        }
    }
}

fn collect_authorized_effects_from_external(
    e: &Env,
    set: &mut Map<Bytes, ()>,
    external: &Vec<(Address, i128, Vec<Condition>)>,
) {
    for (_addr, _amount, conditions) in external.iter() {
        for cond in conditions.iter() {
            if is_execution_bound(&cond) {
                set.set(cond.to_xdr(e), ());
            }
        }
    }
}

/// Only `Create` and `ExtWithdraw` conditions describe on-ledger value movement the bundle
/// executes; they are the effects this binding governs.
fn is_execution_bound(cond: &Condition) -> bool {
    matches!(cond, Condition::Create(..) | Condition::ExtWithdraw(..))
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

pub fn op_has_no_conflicting_conditions(e: &Env, op: &ChannelOperation) -> bool {
    let mut verified_conditions: Vec<Condition> = Vec::new(&e);

    let mut conditions_to_check: Vec<Condition> = Vec::new(&e);
    conditions_to_check.extend(op.spend.iter().flat_map(|(_, conds)| conds.clone()));
    conditions_to_check.extend(op.deposit.iter().flat_map(|(_, _, conds)| conds.clone()));
    conditions_to_check.extend(op.withdraw.iter().flat_map(|(_, _, conds)| conds.clone()));

    for c in conditions_to_check.iter() {
        let cond = c.clone();
        if !condition_does_not_conflict_with_set(&cond, &verified_conditions) {
            return false;
        }
        verified_conditions.push_back(cond);
    }

    true
}
