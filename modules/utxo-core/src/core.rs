use moonlight_auth::core::{AuthRequirements, Condition};
use soroban_sdk::{
    assert_with_error, contracterror, contracttrait, contracttype, panic_with_error, vec, Bytes,
    BytesN, Env, IntoVal, Symbol, Vec,
};

use soroban_sdk::symbol_short;

#[cfg(not(feature = "no-bundle-events"))]
use crate::events::BundleEvent;
use crate::events::UtxoEvent;

#[derive(Clone)]
#[contracttype]
pub enum UTXOCoreDataKey {
    UTXO(BytesN<32>), // 32-byte hash of 65-byte pubkey to reduce storage costs
}

#[derive(Clone)]
#[contracttype]
pub enum UtxoState {
    Unspent(i128), // takes 1-byte tag + 16 bytes value
    Spent,         // only 1-byte tag (optimizing for read/write size)
}

#[derive(Clone)]
#[contracttype]
pub struct Bundle {
    pub spend: Vec<BytesN<65>>,
    pub create: Vec<(BytesN<65>, i128)>,
    pub req: AuthRequirements,
}

#[derive(Clone)]
#[contracttype]
pub struct OperationBundle {
    pub spend: Vec<(BytesN<65>, Vec<Condition>)>,
    pub create: Vec<(BytesN<65>, i128)>,
}

pub const STORAGE_KEY_UTXO_AUTH: &Symbol = &symbol_short!("UTXO_AUTH");

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    UTXOAlreadyExists = 1,
    UTXODoesntExist = 2,
    UTXOAlreadySpent = 3,
    UnbalancedBundle = 4,
    InvalidCreateAmount = 5,
    RepeatedCreateUTXO = 6,
    RepeatedSpendUTXO = 7,
}

#[contracttrait]
pub trait UtxoHandlerTrait {
    fn utxo_auth(env: &Env) -> soroban_sdk::Address {
        env.storage()
            .instance()
            .get(STORAGE_KEY_UTXO_AUTH)
            .unwrap_or_else(|| panic!("UTXO auth not set"))
    }

    fn set_utxo_auth(env: &Env, new_auth: &soroban_sdk::Address) {
        env.storage()
            .instance()
            .set(STORAGE_KEY_UTXO_AUTH, new_auth);
    }

    /// Returns the balance of a given UTXO.
    ///
    /// If the UTXO is unspent, the stored balance is returned.
    /// If the UTXO is spent, 0 is returned.
    /// If no record exists for the UTXO (represented by –1), it is considered free to be created.
    fn utxo_balance(e: Env, utxo: BytesN<65>) -> i128 {
        match e
            .storage()
            .persistent()
            .get::<_, UtxoState>(&UTXOCoreDataKey::UTXO(Self::hash_utxo_key(&e, &utxo)))
        {
            Some(UtxoState::Unspent(amount)) => amount,
            Some(UtxoState::Spent) => 0,
            None => -1,
        }
    }

    #[internal]
    fn process_bundle(
        e: Env,
        bundle: Bundle,
        incoming_amount: i128,
        expected_outgoing: i128,
    ) -> i128 {
        let mut total_available_balance = incoming_amount;

        if !no_duplicate_keys(&e, bundle.spend.iter(), |spend_utxo| spend_utxo.clone()) {
            panic_with_error!(&e, Error::RepeatedSpendUTXO);
        }
        if !no_duplicate_keys(&e, bundle.create.iter(), |(create_utxo, _amt)| {
            create_utxo.clone()
        }) {
            panic_with_error!(&e, Error::RepeatedCreateUTXO);
        }

        Self::utxo_auth(&e).require_auth_for_args(vec![&e, bundle.req.clone().into_val(&e)]);

        for spend_utxo in bundle.spend.iter() {
            let unspent_balance = Self::spend(&e, spend_utxo.clone());
            total_available_balance += unspent_balance;
        }

        for (create_utxo, amount) in bundle.create.iter() {
            Self::create(&e, amount, create_utxo.clone());

            total_available_balance -= amount;
        }

        assert_with_error!(
            &e,
            total_available_balance == expected_outgoing,
            Error::UnbalancedBundle
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
    /// Creates a new UTXO associated with the given balance. The UTXO must not already exist.
    ///
    ///### Panics
    /// - Panics if the UTXO already exists.
    #[internal]
    fn create(e: &Env, amount: i128, utxo: BytesN<65>) {
        Self::verify_utxo_not_exists(&e, utxo.clone());

        assert_with_error!(&e, amount > 0, Error::InvalidCreateAmount);

        Self::unchecked_create(e, amount, utxo);
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
    #[internal]
    fn spend(e: &Env, utxo: BytesN<65>) -> i128 {
        let amount = Self::verify_utxo_unspent(&e, utxo.clone());
        Self::unchecked_spend(&e, utxo.clone(), amount);
        amount
    }

    #[internal]
    fn unchecked_create(e: &Env, amount: i128, utxo: BytesN<65>) {
        let key = UTXOCoreDataKey::UTXO(Self::hash_utxo_key(&e, &utxo));
        e.storage()
            .persistent()
            .set(&key, &UtxoState::Unspent(amount));
        UtxoEvent {
            name: symbol_short!("utxo"),
            utxo,
            action: symbol_short!("create"),
            amount,
        }
        .publish(&e);
    }

    #[internal]
    fn unchecked_spend(e: &Env, utxo: BytesN<65>, _amount: i128) {
        let key = UTXOCoreDataKey::UTXO(Self::hash_utxo_key(&e, &utxo));
        e.storage().persistent().set(&key, &UtxoState::Spent);
        UtxoEvent {
            name: symbol_short!("utxo"),
            utxo,
            action: symbol_short!("spend"),
            amount: _amount,
        }
        .publish(&e);
    }

    // hash the UTXO key to reduce storage costs
    // by using a 32-byte hash instead of a 65-byte pubkey
    // this doesn't affect the behavior of the contract
    #[internal]
    fn hash_utxo_key(e: &Env, utxo: &BytesN<65>) -> BytesN<32> {
        let utxo_bytes = Bytes::from_slice(&e, utxo.to_array().as_ref());
        let hash = e.crypto().sha256(&utxo_bytes);
        BytesN::<32>::from_array(&e, &hash.to_array())
    }

    #[internal]
    fn verify_utxo_not_exists(e: &Env, utxo: BytesN<65>) {
        let key = UTXOCoreDataKey::UTXO(Self::hash_utxo_key(&e, &utxo));

        assert_with_error!(
            &e,
            e.storage().persistent().get::<_, UtxoState>(&key).is_none(),
            Error::UTXOAlreadyExists
        );
    }

    #[internal]
    fn verify_utxo_unspent(e: &Env, utxo: BytesN<65>) -> i128 {
        let key = UTXOCoreDataKey::UTXO(Self::hash_utxo_key(&e, &utxo));

        match e.storage().persistent().get::<_, UtxoState>(&key) {
            Some(UtxoState::Unspent(amount)) => amount,
            Some(UtxoState::Spent) => panic_with_error!(&e, Error::UTXOAlreadySpent),
            None => panic_with_error!(&e, Error::UTXODoesntExist),
        }
    }
}

// Returns true if all keys produced by key_fn are unique, false if any duplicate appears.
fn no_duplicate_keys<I, F>(e: &Env, iter: I, mut key_fn: F) -> bool
where
    I: IntoIterator,
    F: FnMut(I::Item) -> BytesN<65>,
{
    let mut seen: soroban_sdk::Map<BytesN<65>, bool> = soroban_sdk::Map::new(e);
    for item in iter {
        let k = key_fn(item);
        if seen.contains_key(k.clone()) {
            return false;
        }
        seen.set(k, true);
    }
    true
}
