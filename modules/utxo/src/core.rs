use soroban_sdk::{contracttype, crypto::Hash, Bytes, BytesN, Env, Vec};

#[cfg(not(all(feature = "no-utxo-events", feature = "no-delegate-events")))]
use soroban_sdk::symbol_short;

use crate::emit_optional_event;

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
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
    pub signatures: Vec<BytesN<64>>,
}

#[derive(Clone)]
#[contracttype]
pub struct MintRequest {
    pub utxo: BytesN<65>,
    pub amount: i128,
}

#[derive(Clone)]
#[contracttype]
pub struct BurnRequest {
    pub utxo: BytesN<65>,
    pub signature: BytesN<64>,
}

/// Returns the balance of a given UTXO.
///
/// If the UTXO is unspent, the stored balance is returned.
/// If the UTXO is spent, 0 is returned.
/// If no record exists for the UTXO (represented by –1), it is considered free to be created.
pub fn utxo_balance(e: Env, utxo: BytesN<65>) -> i128 {
    match e
        .storage()
        .persistent()
        .get::<_, UtxoState>(&DataKey::UTXO(hash_utxo_key(&e, &utxo)))
    {
        Some(UtxoState::Unspent(amount)) => amount,
        Some(UtxoState::Spent) => 0,
        None => -1,
    }
}

/// Mints a new UTXO with the specified balance.
///
/// Creates a new UTXO associated with the given balance. The UTXO must not already exist.
///
///### Panics
/// - Panics if the UTXO already exists.
pub fn mint(e: &Env, amount: i128, utxo: BytesN<65>) {
    create(&e, amount, utxo);
}

/// Mints multiple UTXOs in a single call.
///
/// Processes each mint request—each containing a UTXO identifier and a balance—sequentially,
/// ensuring that no UTXO is minted more than once.
///
///### Panics
/// - Panics if any UTXO in the batch already exists.
pub fn mint_batch(e: &Env, requests: Vec<MintRequest>) {
    for req in requests.iter() {
        mint(e, req.amount, req.utxo.clone());
    }
}

/// Burns the specified UTXO after verifying its authorization signature.
///
/// This function requires an ECDSA signature over a burn payload that is deterministically derived
/// by concatenating the literal "BURN", the UTXO’s 65-byte public key, and the amount (as an 8-byte little-endian value).
/// The signature must be generated using the secret key corresponding to the UTXO's public key, and is verified using secp256r1.
///
/// ### Panics
/// - Panics if signature verification fails.
/// - Panics if the UTXO is already spent or does not exist.
pub fn burn(e: &Env, utxo: BytesN<65>, signature: BytesN<64>) {
    verify_burn_signature(&e, &utxo, &signature);
    spend(&e, utxo);
}

/// Burns multiple UTXOs in a single call.
///
/// Each burn request consists of a UTXO identifier and a corresponding signature.
/// The signature for each UTXO must be generated using the secret key corresponding to the UTXO's public key,
/// and must be valid over the burn payload (which is derived from "BURN", the UTXO’s public key, and the amount).
///
/// ### Panics
/// - Panics if any signature verification fails.
/// - Panics if any UTXO is already spent or does not exist.
pub fn burn_batch(e: &Env, requests: Vec<BurnRequest>) {
    for req in requests.iter() {
        burn(e, req.utxo.clone(), req.signature.clone());
    }
}

/// Executes atomic multi-UTXO transfers by processing each bundle.
///
/// For every bundle, it verifies that the signatures match the "TRANSFER" action and then
/// processes the bundle so that the total value spent equals the total value created. This ensures that
/// each transfer operation is fully authorized and balanced.
///
/// ### Panics
/// - Panics if any bundle is unbalanced (i.e. the total spent does not equal the total created).
pub fn transfer(e: &Env, bundles: Vec<Bundle>) {
    for bundle in bundles.iter() {
        verify_bundle_signatures(&e, &bundle, "TRANSFER");
        let unused_balance = bundle_transfer(e.clone(), bundle.clone());
        assert!(unused_balance == 0, "The bundle do not balance properly!");
    }
}

/// Processes multiple bundles and returns any leftover funds from the transfers.
///
/// For each bundle, the function verifies the bundle's signatures using the specified action,
/// then calculates the difference between the total funds spent and the total funds created. This difference
/// (the leftover) represents any unassigned funds in the transaction. If these leftover funds are not handled
/// by the caller, they are effectively burned.
///
/// ### Panics
/// - Panics if any bundle creates more than it spends.
pub fn transfer_burn_leftover(e: &Env, bundles: Vec<Bundle>, action: &str) -> i128 {
    let mut leftover = 0;

    for bundle in bundles.iter() {
        verify_bundle_signatures(&e, &bundle, action);
        let unused_balance = bundle_transfer(e.clone(), bundle.clone());
        assert!(
            unused_balance >= 0,
            "The bundle is creating more than it is spending!"
        );
        leftover += unused_balance;
    }

    leftover
}

/// Executes delegated transfers by processing multiple bundles where leftover funds are collected as a fee.
/// Each bundle is verified with the "DELEGATED_TRANSFER" action, ensuring that the signed data authorizes the operation.
/// Leftover funds (i.e. the difference between the total spent and the total created in each bundle) are summed
/// and used to create a delegate UTXO. An event is then emitted with the fee amount.
///
/// ### Panics
/// - Panics if any bundle creates more than it spends.
/// - Panics if there are no delegated funds (i.e. leftover is negative or zero).
pub fn delegated_transfer(e: &Env, bundles: Vec<Bundle>, delegate_utxo: BytesN<65>) {
    let delegate_funds = transfer_burn_leftover(e, bundles, "DELEGATED_TRANSFER");

    assert!(
        delegate_funds >= 0,
        "There are no delegated funds in these bundles!"
    );

    create(&e, delegate_funds.clone(), delegate_utxo.clone());
    emit_optional_event!(
        "delegate",
        e,
        delegate_utxo,
        symbol_short!("fee"),
        delegate_funds
    );
}

/// Constructs the payload for burning a UTXO.
///
/// The payload is built by concatenating:
/// - The literal "BURN" (4 bytes),
/// - The 65-byte public key associated with the UTXO,
/// - The 8-byte little-endian representation of the UTXO's amount.
///
/// This payload is then hashed (using SHA-256) to produce a digest that is used for signature verification.
pub fn burn_payload(e: &Env, utxo: &BytesN<65>, amount: i128) -> Hash<32> {
    let mut b = Bytes::new(&e);
    b.append(&Bytes::from_slice(&e, b"BURN"));
    b.append(&Bytes::from_slice(&e, utxo.to_array().as_ref()));
    b.append(&Bytes::from_slice(&e, &amount.to_le_bytes()));

    e.crypto().sha256(&b)
}

/// Constructs the payload for processing a bundle of UTXO operations.
///
/// The payload is built by concatenating:
/// - The literal "BUNDLE" (6 bytes),
/// - The provided action string (e.g., "TRANSFER", "DELEGATED_TRANSFER", "CUSTOM"),
/// - For each UTXO in the `spend` list: its 65-byte public key,
/// - For each entry in the `create` list: the UTXO's 65-byte public key followed by its 8-byte little-endian amount.
///
/// The resulting byte stream is hashed using SHA-256 to produce a digest that is used for verifying the signatures of the bundle.
pub fn bundle_payload(e: &Env, bundle: Bundle, action: &str) -> Hash<32> {
    let mut b = Bytes::new(&e);
    b.append(&Bytes::from_slice(&e, b"BUNDLE"));
    b.append(&Bytes::from_slice(e, action.as_bytes()));
    for utxo in bundle.spend.iter() {
        b.append(&Bytes::from_slice(&e, utxo.to_array().as_ref()));
    }
    for (utxo, amount) in bundle.create.iter() {
        b.append(&Bytes::from_slice(&e, utxo.to_array().as_ref()));
        b.append(&Bytes::from_slice(&e, &amount.to_le_bytes()));
    }

    e.crypto().sha256(&b)
}

// ------------------------------------------
// PRIVATE FUNCTIONS
// ------------------------------------------

fn unchecked_create(e: &Env, amount: i128, utxo: BytesN<65>) {
    let key = DataKey::UTXO(hash_utxo_key(&e, &utxo));
    e.storage()
        .persistent()
        .set(&key, &UtxoState::Unspent(amount));
    emit_optional_event!("utxo", e, utxo, symbol_short!("create"), amount);
}

fn create(e: &Env, amount: i128, utxo: BytesN<65>) {
    verify_utxo_not_exists(&e, utxo.clone());
    unchecked_create(e, amount, utxo);
}

fn unchecked_spend(e: &Env, utxo: BytesN<65>, _amount: i128) {
    let key = DataKey::UTXO(hash_utxo_key(&e, &utxo));
    e.storage().persistent().set(&key, &UtxoState::Spent);
    emit_optional_event!("utxo", e, utxo, symbol_short!("spend"), _amount);
}

fn spend(e: &Env, utxo: BytesN<65>) -> i128 {
    let amount = verify_utxo_unspent(&e, utxo.clone());
    unchecked_spend(&e, utxo.clone(), amount);
    amount
}

fn bundle_transfer(e: Env, bundle: Bundle) -> i128 {
    if bundle.spend.len() != bundle.signatures.len() {
        panic!("Bundle has mismatched spend and signature lengths");
    }

    if bundle.spend.len() == 0 {
        panic!("Bundle must have at least one spend UTXO"); // create is not enforced as it can be managed by the caller for other purposes
    }

    let mut bundle_funds = 0;
    for utxo in bundle.spend.iter() {
        let unspent_balance = verify_utxo_unspent(&e, utxo.clone());
        unchecked_spend(&e, utxo.clone(), unspent_balance); // verified above
        bundle_funds += unspent_balance;
    }

    for (utxo, amount) in bundle.create.iter() {
        create(&e, amount, utxo.clone());
        bundle_funds -= amount;
    }

    bundle_funds
}

fn read_utxo(e: &Env, utxo: &BytesN<65>) -> i128 {
    let key = DataKey::UTXO(hash_utxo_key(&e, &utxo));
    match e.storage().persistent().get::<_, UtxoState>(&key) {
        Some(UtxoState::Unspent(amount)) => amount,
        _ => -1, // either Spent or None
    }
}

// hash the UTXO key to reduce storage costs
// by using a 32-byte hash instead of a 65-byte pubkey
// this doesn't affect the behavior of the contract
fn hash_utxo_key(e: &Env, utxo: &BytesN<65>) -> BytesN<32> {
    let utxo_bytes = Bytes::from_slice(&e, utxo.to_array().as_ref());
    let hash = e.crypto().sha256(&utxo_bytes);
    BytesN::<32>::from_array(&e, &hash.to_array())
}

fn verify_utxo_not_exists(e: &Env, utxo: BytesN<65>) {
    let key = DataKey::UTXO(hash_utxo_key(&e, &utxo));
    if e.storage().persistent().get::<_, UtxoState>(&key).is_some() {
        panic!("UTXO already exists");
    }
}

// fn verify_utxo_exists(e: &Env, utxo: BytesN<65>) {
//     let key = DataKey::UTXO(hash_utxo_key(&e, &utxo));
//     if e.storage().persistent().get::<_, UtxoState>(&key).is_none() {
//         panic!("UTXO does not exist");
//     }
// }

fn verify_utxo_unspent(e: &Env, utxo: BytesN<65>) -> i128 {
    let key = DataKey::UTXO(hash_utxo_key(&e, &utxo));
    match e.storage().persistent().get::<_, UtxoState>(&key) {
        Some(UtxoState::Unspent(amount)) => amount,
        _ => panic!("UTXO spent or nonexistent"),
    }
}

fn verify_burn_signature(e: &Env, utxo: &BytesN<65>, signature: &BytesN<64>) {
    let amount = read_utxo(&e, &utxo);
    let hash = burn_payload(&e, utxo, amount);
    verify_secp256r1_signature(&e, utxo, signature, &hash);
}

fn verify_bundle_signatures(e: &Env, bundle: &Bundle, action: &str) {
    let hash = bundle_payload(&e, bundle.clone(), action);
    for (i, utxo) in bundle.spend.iter().enumerate() {
        let sig = bundle.signatures.get(i as u32).unwrap(); // Soroban Vec uses u32
        verify_secp256r1_signature(&e, &utxo, &sig, &hash);
    }
}

fn verify_secp256r1_signature(
    e: &Env,
    public_key: &BytesN<65>,
    signature: &BytesN<64>,
    payload_hash: &Hash<32>,
) {
    e.crypto()
        .secp256r1_verify(public_key, payload_hash, signature);
}
