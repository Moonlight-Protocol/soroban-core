use moonlight_errors::Error as MoonlightError;
use moonlight_primitives::{no_duplicate_keys, AuthRequirements, Condition, SignerKey};
use soroban_sdk::{
    assert_with_error, contracttype, panic_with_error, vec, BytesN, Env, IntoVal, Map, Symbol, Vec,
};

use moonlight_storage::Store;

use soroban_sdk::symbol_short;

#[cfg(not(feature = "no-bundle-events"))]
use crate::events::BundleEvent;
#[cfg(not(feature = "no-utxo-events"))]
use crate::events::UtxoEvent;

#[derive(Clone)]
#[contracttype]
pub struct InternalBundle {
    pub spend: Vec<BytesN<65>>,
    pub create: Vec<(BytesN<65>, i128)>,
    pub req: AuthRequirements,
}

#[derive(Clone)]
#[contracttype]
pub struct UTXOOperation {
    pub spend: Vec<(BytesN<65>, Vec<Condition>)>,
    pub create: Vec<(BytesN<65>, i128)>,
}

pub const STORAGE_KEY_UTXO_AUTH: &Symbol = &symbol_short!("UTXO_AUTH");

pub trait UtxoHandlerTrait {
    fn auth(env: &Env) -> soroban_sdk::Address {
        env.storage()
            .instance()
            .get(STORAGE_KEY_UTXO_AUTH)
            .unwrap_or_else(|| panic_with_error!(env, MoonlightError::AuthContractNotSet))
    }

    fn set_auth(env: &Env, new_auth: &soroban_sdk::Address) {
        env.storage()
            .instance()
            .set(STORAGE_KEY_UTXO_AUTH, new_auth);
    }

    /// Returns the balance of a given UTXO.
    ///
    /// If the UTXO is unspent, the stored balance is returned.
    /// If the UTXO is spent, 0 is returned.
    /// If no record exists for the UTXO (represented by –1), it is considered free to be created.
    fn utxo_balance(e: &Env, utxo: BytesN<65>) -> i128 {
        Store::apply(e, |store| store.balance(&utxo))
    }
    fn utxo_balances(e: &Env, utxos: Vec<BytesN<65>>) -> Vec<i128> {
        let mut balances: Vec<i128> = vec![&e];

        for u in utxos {
            balances.push_back(Self::utxo_balance(&e, u));
        }

        balances
    }

    fn process_bundle(
        e: &Env,
        bundle: InternalBundle,
        incoming_amount: i128,
        expected_outgoing: i128,
    ) -> i128 {
        let mut total_available_balance = incoming_amount;

        assert_with_error!(
            &e,
            no_duplicate_keys(&e, bundle.spend.iter(), |spend_utxo| spend_utxo.clone()),
            MoonlightError::RepeatedSpendUtxo
        );

        assert_with_error!(
            &e,
            no_duplicate_keys(&e, bundle.create.iter(), |(create_utxo, _amt)| {
                create_utxo.clone()
            }),
            MoonlightError::RepeatedCreateUtxo
        );

        let auth_args = if bundle.req.0.is_empty() {
            vec![&e]
        } else {
            vec![&e, bundle.req.clone().into_val(e)]
        };

        Self::auth(&e).require_auth_for_args(auth_args);

        Store::apply(e, |store| {
            for spend_utxo in bundle.spend.iter() {
                let amount = match store.balance(&spend_utxo) {
                    a if a > 0 => a,
                    0 => panic_with_error!(e, MoonlightError::UtxoAlreadySpent),
                    _ => panic_with_error!(e, MoonlightError::UtxoDoesNotExist),
                };

                store.spend(&spend_utxo);
                total_available_balance += amount;

                #[cfg(not(feature = "no-utxo-events"))]
                UtxoEvent {
                    name: symbol_short!("utxo"),
                    utxo: spend_utxo.clone(),
                    action: symbol_short!("spend"),
                    amount,
                }
                .publish(&e);
            }

            for (create_utxo, amount) in bundle.create.iter() {
                if store.balance(&create_utxo) != -1 {
                    panic_with_error!(e, MoonlightError::UtxoAlreadyExists);
                }

                assert_with_error!(&e, amount > 0, MoonlightError::InvalidCreateAmount);

                store.create(&create_utxo, amount);
                total_available_balance -= amount;

                #[cfg(not(feature = "no-utxo-events"))]
                UtxoEvent {
                    name: symbol_short!("utxo"),
                    utxo: create_utxo.clone(),
                    action: symbol_short!("create"),
                    amount,
                }
                .publish(&e);
            }
        });

        assert_with_error!(
            &e,
            total_available_balance == expected_outgoing,
            MoonlightError::UnbalancedBundle
        );

        #[cfg(not(feature = "no-bundle-events"))]
        BundleEvent {
            name: soroban_sdk::symbol_short!("bundle"),
            spend: bundle.spend.clone(),
            create: bundle.create.clone(),
            deposited: incoming_amount,
            withdrawn: expected_outgoing,
        }
        .publish(&e);

        total_available_balance

        // bundle_funds
    }

    /// Creates a new UTXO with the specified balance after verifying it does not already exist.
    ///
    ///### Panics
    /// - Panics if the UTXO already exists.
    fn create(e: &Env, amount: i128, utxo: BytesN<65>) {
        Self::verify_utxo_not_exists(&e, utxo.clone());

        assert_with_error!(&e, amount > 0, MoonlightError::InvalidCreateAmount);

        Self::unchecked_create(e, amount, &utxo);
    }

    /// Spends the specified UTXO after verifying its state.
    ///
    /// This function requires an ECDSA signature over a burn payload that is deterministically derived
    /// by concatenating the literal "BURN", the UTXO’s 65-byte public key, and the amount (as an 8-byte little-endian value).
    /// The signature must be generated using the secret key corresponding to the UTXO's public key, and is verified using secp256r1.
    ///
    /// ### Panics
    /// - Panics if signature verification fails.
    /// - Panics if the UTXO is already spent or does not exist.
    fn spend(e: &Env, utxo: &BytesN<65>) -> i128 {
        let amount = Self::verify_utxo_unspent(&e, utxo.clone());
        Self::unchecked_spend(&e, utxo.clone(), amount);
        amount
    }

    fn unchecked_create(e: &Env, amount: i128, utxo: &BytesN<65>) {
        Store::apply(e, |store| store.create(utxo, amount));

        #[cfg(not(feature = "no-utxo-events"))]
        UtxoEvent {
            name: symbol_short!("utxo"),
            utxo: utxo.clone(),
            action: symbol_short!("create"),
            amount,
        }
        .publish(&e);
    }

    fn unchecked_spend(e: &Env, utxo: BytesN<65>, _amount: i128) {
        Store::apply(e, |store| {
            store.spend(&utxo);
        });

        #[cfg(not(feature = "no-utxo-events"))]
        UtxoEvent {
            name: symbol_short!("utxo"),
            utxo,
            action: symbol_short!("spend"),
            amount: _amount,
        }
        .publish(&e);
    }

    fn verify_utxo_not_exists(e: &Env, utxo: BytesN<65>) {
        if Store::apply(e, |store| store.balance(&utxo)) != -1 {
            panic_with_error!(e, MoonlightError::UtxoAlreadyExists);
        }
    }

    fn verify_utxo_unspent(e: &Env, utxo: BytesN<65>) -> i128 {
        match Store::apply(e, |store| store.balance(&utxo)) {
            a if a > 0 => a,
            0 => panic_with_error!(e, MoonlightError::UtxoAlreadySpent),
            _ => panic_with_error!(e, MoonlightError::UtxoDoesNotExist),
        }
    }
}

// This should be different depending on the contract impl
pub fn calculate_auth_requirements(
    e: &Env,
    p256: &Vec<(BytesN<65>, Vec<Condition>)>,
) -> AuthRequirements {
    let mut map_req: Map<SignerKey, Vec<Condition>> = Map::new(&e);

    for (spend_utxo, conditions) in p256.iter() {
        map_req.set(SignerKey::P256(spend_utxo.clone()), conditions.clone());
    }

    AuthRequirements(map_req)
}
