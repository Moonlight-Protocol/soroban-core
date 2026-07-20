use moonlight_errors::Error;
use moonlight_primitives::{
    condition_does_not_conflict_with_set, equal_condition_sequence, no_duplicate_addresses,
    Condition,
};
use moonlight_utxo_core::core::{calculate_auth_requirements, InternalBundle};
use soroban_sdk::{
    assert_with_error,
    auth::{ContractContext, InvokerContractAuthEntry, SubContractInvocation},
    contracttype, panic_with_error,
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
    // All condition/operation validation is consolidated in `validate_channel_operation`, which
    // runs every check in the exact required order and returns the deposit/withdraw totals it
    // computes while validating the external amounts.
    let (total_deposit, total_withdraw) = validate_channel_operation(e, &op);

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

/// Consolidated validation for a `ChannelOperation`.
///
/// This is the single place that owns every condition/operation check the channel enforces before a
/// bundle is processed. It runs the checks in the exact order the contract requires and returns
/// `(total_deposit, total_withdraw)` — the external-amount totals it necessarily computes while
/// validating those amounts.
///
/// The order is behavior-significant: it fixes which error is surfaced when an input violates more
/// than one rule. In particular the amount/overflow checks (B/C) run *before* the
/// duplicate-address / cross-side checks (D), so an input that both overflows and repeats an address
/// still reports `AmountOverflow` rather than `RepeatedAccountForDeposit`.
///
/// Sequence (each check preserved 1:1 from its previous scattered location):
/// 1. A — no conflicting conditions across `spend ∪ deposit ∪ withdraw`
///    (`BundleHasConflictingConditions`). See `op_has_no_conflicting_conditions`.
/// 2. B — each deposit amount is strictly positive (`InvalidExternalAmount`) and the running sum
///    does not overflow (`AmountOverflow`); accumulates `total_deposit`.
/// 3. C — same for withdraw amounts; accumulates `total_withdraw`.
/// 4. D — no duplicate deposit/withdraw addresses, and for any address present on both sides the two
///    condition sequences are XDR-equal (`RepeatedAccountForDeposit` / `RepeatedAccountForWithdraw`
///    / `ConflictingConditionsForAccount`). See `verify_external_operations`.
/// 5. E — every owner-signed `Create` / `ExtWithdraw` effect is executed (`UnauthorizedOperation`,
///    MOON-01). See `assert_signed_effects_are_executed`.
fn validate_channel_operation(e: &Env, op: &ChannelOperation) -> (i128, i128) {
    // A: no conflicting conditions across the flattened spend/deposit/withdraw condition sets.
    assert_with_error!(
        e,
        op_has_no_conflicting_conditions(e, op),
        Error::BundleHasConflictingConditions
    );

    // B: deposit amounts strictly positive; running sum non-overflowing.
    let mut total_deposit: i128 = 0;
    for (_addr, amt, _conds) in op.deposit.iter() {
        // MOON-05: reject non-positive amounts in-contract rather than relying on the asset SAC.
        assert_with_error!(e, amt > 0, Error::InvalidExternalAmount);
        total_deposit = match total_deposit.checked_add(amt) {
            Some(v) => v,
            None => panic_with_error!(e, Error::AmountOverflow),
        };
    }

    // C: withdraw amounts strictly positive; running sum non-overflowing.
    let mut total_withdraw: i128 = 0;
    for (_addr, amt, _conds) in op.withdraw.iter() {
        assert_with_error!(e, amt > 0, Error::InvalidExternalAmount);
        total_withdraw = match total_withdraw.checked_add(amt) {
            Some(v) => v,
            None => panic_with_error!(e, Error::AmountOverflow),
        };
    }

    // D: no duplicate deposit/withdraw addresses; cross-side condition-sequence equality.
    verify_external_operations(e, op.deposit.clone(), op.withdraw.clone());

    // E: MOON-01 — bind owner-signed conditions to executed effects. The balance check in
    // `process_bundle` only guarantees value conservation, not *where* the value goes; without
    // this, a provider could keep `op.spend` byte-identical (owner P256 sig still verifies) and
    // redirect `op.create` / `op.withdraw` to an attacker while staying balanced.
    assert_signed_effects_are_executed(e, op);

    (total_deposit, total_withdraw)
}

/// MOON-01 binding: enforce that every cryptographically-signed `Create` / `ExtWithdraw` condition
/// is executed exactly by the bundle (subset direction `authorized ⊆ executed`).
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
/// Compared by canonical XDR bytes with set (dedup) semantics, so a multi-spend bundle where each
/// spend repeats (or partitions) the output set is accepted as long as every signed effect appears
/// in the executed effects. Conditions can deliberately repeat and overlap across multiple
/// operations — this helps obfuscate which condition belongs to which signer and opens up
/// different composition strategies. A signer's outputs still cannot be dropped, reduced,
/// amount-changed, or redirected (any of those removes the exact `(utxo|addr, amount)` key from
/// `executed`).
///
/// ### Batching constraint (RV audit A3)
/// The set semantics collapse byte-identical conditions to one entry — the contract considers them
/// one single operation: if two *independently signed* spends each demand the identical
/// `ExtWithdraw(to, amount)`, a bundle containing both discharges them with a single executed
/// withdrawal (e.g. two conditions each for a withdraw of 10 tokens to address A enforce one
/// single withdraw of 10), while the balance check still counts both spent amounts in full — the difference becomes residual the composer allocates like a fee. This is NOT
/// enforced against in-contract; it is an accepted trust-boundary constraint on the composer
/// (already a semi-trusted role — see PC-14 / A3 in `contracts/arch.md`):
/// - Byte-identical execution-bound conditions from distinct signers must never share a bundle.
/// - Screening co-batched conditions for byte-identical collisions is the submitter/composer's
///   responsibility.
/// - Recurring or shared identical effects (several parties withdrawing a common amount to a
///   shared address; fixed-amount recurring payments) settle safely only in separate bundles.
///
/// EXTRA executed creates/withdraws beyond the signed set are permitted — this is the provider fee.
/// The retained balance check (`Σexecuted == Σinputs`) bounds those extras to exactly the residual
/// the signers left unallocated (deposit over-funded / spend under-claimed); for a pure internal
/// transfer the residual is zero, so no extra create can balance.
///
/// ### Panics
/// - `UnauthorizedOperation` if a signed create/withdraw condition is not executed.
fn assert_signed_effects_are_executed(e: &Env, op: &ChannelOperation) {
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

    // Subset: every signed effect must be executed exactly. Extra executed effects are allowed.
    for key in authorized.keys().iter() {
        assert_with_error!(e, executed.contains_key(key), Error::UnauthorizedOperation);
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

    // B11: a withdrawal naming the channel itself would execute as
    // `transfer(channel, channel, amount)` — a net-zero token move — while `Supply` still
    // decrements, burning UTXO claims without any tokens leaving. The resulting surplus
    // (token balance above `Supply`) is unreachable through `transact`, so reject the
    // self-withdraw outright. The direct-transfer variant — sending tokens straight to the
    // channel address outside `transact` — cannot be prevented here; such tokens are inert
    // surplus above `Supply`, recoverable only by an admin upgrade.
    for (to, _amount, _conditions) in withdraw.iter() {
        if to == e.current_contract_address() {
            panic_with_error!(&e, Error::WithdrawToChannelAddress);
        }
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
        // B10: the channel requires `from` to authorize only `[conditions]`; consent to the
        // deposit `amount` is delegated to the asset's `transfer`, which requires `from`'s
        // authorization for the exact `(from, to, amount)` triple. This is sound iff the
        // channel asset requires `from`'s authorization to move value (true for a compliant
        // SAC). An asset that can move value without `from`'s authorization must not be used
        // as a channel asset — nothing at the channel layer binds the amount to the
        // depositor's consent (see `contracts/arch.md`, Asset SAC trust assumption).
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

/// Equivalence backstop for the consolidation of condition/operation validation into
/// `validate_channel_operation`. These tests pin the accept/reject outcome, the exact error code,
/// and — critically — the check *ordering* that determines which error surfaces when an input
/// violates more than one rule. Reject cases go through `try_transact` (validation runs before any
/// signature check, so no signatures are needed) and assert the exact contract error, exactly as
/// the pre-existing MOON-01 / MOON-05 tests do.
#[cfg(test)]
mod validation_tests {
    use super::{op_has_no_conflicting_conditions, validate_channel_operation, ChannelOperation};
    use crate::test::test::create_contracts;
    use moonlight_errors::Error as ContractError;
    use moonlight_helpers::testutils::snapshot::get_env_with_g_accounts;
    use moonlight_primitives::Condition;
    use soroban_sdk::{testutils::Address as _, vec, Address, Env, Error as SdkError};

    fn assert_channel_error(
        res_err: Option<Result<SdkError, soroban_sdk::InvokeError>>,
        expected: ContractError,
    ) {
        assert_eq!(
            res_err,
            Some(Ok(SdkError::from_contract_error(expected as u32)))
        );
    }

    // ---- Reject cases (one per previously-uncovered check) ----

    // A: two conflicting conditions (same ExtDeposit address, different amount) anywhere in the
    // flattened spend/deposit/withdraw condition sets.
    #[test]
    fn rejects_conflicting_conditions() {
        let e = get_env_with_g_accounts();
        let (channel, _auth, _token, _admin) = create_contracts(&e);
        let d = Address::generate(&e);
        let x = Address::generate(&e);

        let op = ChannelOperation {
            spend: vec![&e],
            create: vec![&e],
            deposit: vec![
                &e,
                (
                    d,
                    100_i128,
                    vec![
                        &e,
                        Condition::ExtDeposit(x.clone(), 1_i128),
                        Condition::ExtDeposit(x, 2_i128),
                    ],
                ),
            ],
            withdraw: vec![&e],
        };

        assert_channel_error(
            channel.try_transact(&op).err(),
            ContractError::BundleHasConflictingConditions,
        );
    }

    // B2: deposit running-sum overflow (distinct addresses so the dup-address check cannot fire).
    #[test]
    fn rejects_deposit_overflow() {
        let e = get_env_with_g_accounts();
        let (channel, _auth, _token, _admin) = create_contracts(&e);
        let a = Address::generate(&e);
        let b = Address::generate(&e);

        let op = ChannelOperation {
            spend: vec![&e],
            create: vec![&e],
            deposit: vec![&e, (a, i128::MAX, vec![&e]), (b, 1_i128, vec![&e])],
            withdraw: vec![&e],
        };

        assert_channel_error(
            channel.try_transact(&op).err(),
            ContractError::AmountOverflow,
        );
    }

    // C2: withdraw running-sum overflow.
    #[test]
    fn rejects_withdraw_overflow() {
        let e = get_env_with_g_accounts();
        let (channel, _auth, _token, _admin) = create_contracts(&e);
        let a = Address::generate(&e);
        let b = Address::generate(&e);

        let op = ChannelOperation {
            spend: vec![&e],
            create: vec![&e],
            deposit: vec![&e],
            withdraw: vec![&e, (a, i128::MAX, vec![&e]), (b, 1_i128, vec![&e])],
        };

        assert_channel_error(
            channel.try_transact(&op).err(),
            ContractError::AmountOverflow,
        );
    }

    // D1: duplicate deposit address.
    #[test]
    fn rejects_duplicate_deposit_address() {
        let e = get_env_with_g_accounts();
        let (channel, _auth, _token, _admin) = create_contracts(&e);
        let a = Address::generate(&e);

        let op = ChannelOperation {
            spend: vec![&e],
            create: vec![&e],
            deposit: vec![&e, (a.clone(), 10_i128, vec![&e]), (a, 20_i128, vec![&e])],
            withdraw: vec![&e],
        };

        assert_channel_error(
            channel.try_transact(&op).err(),
            ContractError::RepeatedAccountForDeposit,
        );
    }

    // D2: duplicate withdraw address.
    #[test]
    fn rejects_duplicate_withdraw_address() {
        let e = get_env_with_g_accounts();
        let (channel, _auth, _token, _admin) = create_contracts(&e);
        let a = Address::generate(&e);

        let op = ChannelOperation {
            spend: vec![&e],
            create: vec![&e],
            deposit: vec![&e],
            withdraw: vec![&e, (a.clone(), 10_i128, vec![&e]), (a, 20_i128, vec![&e])],
        };

        assert_channel_error(
            channel.try_transact(&op).err(),
            ContractError::RepeatedAccountForWithdraw,
        );
    }

    // D3: an address on both sides whose condition sequences are not XDR-equal. The two conditions
    // use different addresses so check A does not flag them as conflicting first.
    #[test]
    fn rejects_cross_side_sequence_mismatch() {
        let e = get_env_with_g_accounts();
        let (channel, _auth, _token, _admin) = create_contracts(&e);
        let shared = Address::generate(&e);
        let y1 = Address::generate(&e);
        let y2 = Address::generate(&e);

        let op = ChannelOperation {
            spend: vec![&e],
            create: vec![&e],
            deposit: vec![
                &e,
                (
                    shared.clone(),
                    10_i128,
                    vec![&e, Condition::ExtWithdraw(y1, 1_i128)],
                ),
            ],
            withdraw: vec![
                &e,
                (shared, 5_i128, vec![&e, Condition::ExtWithdraw(y2, 1_i128)]),
            ],
        };

        assert_channel_error(
            channel.try_transact(&op).err(),
            ContractError::ConflictingConditionsForAccount,
        );
    }

    // ---- Ordering guards (the crux of behavior preservation) ----

    // Overflow + duplicate deposit address together must still report AmountOverflow, because the
    // amount checks (B) run before the duplicate-address check (D). If B/C were moved after D this
    // would regress to RepeatedAccountForDeposit.
    #[test]
    fn order_overflow_reported_before_duplicate_deposit() {
        let e = get_env_with_g_accounts();
        let (channel, _auth, _token, _admin) = create_contracts(&e);
        let a = Address::generate(&e);

        let op = ChannelOperation {
            spend: vec![&e],
            create: vec![&e],
            deposit: vec![&e, (a.clone(), i128::MAX, vec![&e]), (a, 1_i128, vec![&e])],
            withdraw: vec![&e],
        };

        assert_channel_error(
            channel.try_transact(&op).err(),
            ContractError::AmountOverflow,
        );
    }

    // Conflicting conditions + overflow together must report BundleHasConflictingConditions,
    // because the conflict check (A) runs first.
    #[test]
    fn order_conflict_reported_before_overflow() {
        let e = get_env_with_g_accounts();
        let (channel, _auth, _token, _admin) = create_contracts(&e);
        let a = Address::generate(&e);
        let b = Address::generate(&e);
        let x = Address::generate(&e);

        let op = ChannelOperation {
            spend: vec![&e],
            create: vec![&e],
            deposit: vec![
                &e,
                (
                    a,
                    i128::MAX,
                    vec![
                        &e,
                        Condition::ExtDeposit(x.clone(), 1_i128),
                        Condition::ExtDeposit(x, 2_i128),
                    ],
                ),
                (b, 1_i128, vec![&e]),
            ],
            withdraw: vec![&e],
        };

        assert_channel_error(
            channel.try_transact(&op).err(),
            ContractError::BundleHasConflictingConditions,
        );
    }

    // ---- Accept cases ----

    // A benign op passes every check and `validate_channel_operation` returns the amount totals it
    // computed. Called directly (no contract invocation needed) since validation is pure.
    #[test]
    fn accepts_benign_op_and_returns_totals() {
        let e = Env::default();
        let a = Address::generate(&e);
        let b = Address::generate(&e);
        let c = Address::generate(&e);

        let op = ChannelOperation {
            spend: vec![&e],
            create: vec![&e],
            deposit: vec![&e, (a, 10_i128, vec![&e]), (b, 20_i128, vec![&e])],
            withdraw: vec![&e, (c, 5_i128, vec![&e])],
        };

        let (total_deposit, total_withdraw) = validate_channel_operation(&e, &op);
        assert_eq!(total_deposit, 30_i128);
        assert_eq!(total_withdraw, 5_i128);
    }

    // E accept path: a signed ExtWithdraw condition that IS executed by the withdraw list passes
    // the MOON-01 binding.
    #[test]
    fn accepts_matching_signed_withdraw_effect() {
        let e = Env::default();
        let depositor = Address::generate(&e);
        let recipient = Address::generate(&e);

        let op = ChannelOperation {
            spend: vec![&e],
            create: vec![&e],
            deposit: vec![
                &e,
                (
                    depositor,
                    10_i128,
                    vec![&e, Condition::ExtWithdraw(recipient.clone(), 5_i128)],
                ),
            ],
            withdraw: vec![&e, (recipient, 5_i128, vec![&e])],
        };

        let (total_deposit, total_withdraw) = validate_channel_operation(&e, &op);
        assert_eq!(total_deposit, 10_i128);
        assert_eq!(total_withdraw, 5_i128);
    }

    // Check A predicate in isolation: disjoint conditions do not conflict; same-address/different
    // amount conditions do.
    #[test]
    fn op_has_no_conflicting_conditions_distinguishes_conflict() {
        let e = Env::default();
        let x = Address::generate(&e);
        let y = Address::generate(&e);
        let d = Address::generate(&e);

        let disjoint = ChannelOperation {
            spend: vec![&e],
            create: vec![&e],
            deposit: vec![
                &e,
                (
                    d.clone(),
                    1_i128,
                    vec![
                        &e,
                        Condition::ExtDeposit(x.clone(), 1_i128),
                        Condition::ExtDeposit(y, 1_i128),
                    ],
                ),
            ],
            withdraw: vec![&e],
        };
        assert!(op_has_no_conflicting_conditions(&e, &disjoint));

        let conflicting = ChannelOperation {
            spend: vec![&e],
            create: vec![&e],
            deposit: vec![
                &e,
                (
                    d,
                    1_i128,
                    vec![
                        &e,
                        Condition::ExtDeposit(x.clone(), 1_i128),
                        Condition::ExtDeposit(x, 2_i128),
                    ],
                ),
            ],
            withdraw: vec![&e],
        };
        assert!(!op_has_no_conflicting_conditions(&e, &conflicting));
    }
}
