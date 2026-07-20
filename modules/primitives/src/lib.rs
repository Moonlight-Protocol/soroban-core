#![no_std]

use soroban_sdk::{contracttype, crypto::Hash, xdr::ToXdr, Address, Bytes, BytesN, Env, Map, Vec};

#[derive(Clone)]
#[contracttype]
pub enum Condition {
    Create(BytesN<65>, i128),                       // Spend to create new UTXOs
    ExtDeposit(Address, i128),                      // Spend to deposit to an account
    ExtWithdraw(Address, i128),                     // Spend to withdraw to an account
    ExtIntegration(Address, Vec<BytesN<65>>, i128), // contract id of the adapter, the keys to authorize the withdrawal, the amount to deposit
}

impl Condition {
    pub fn conflicts_with(&self, other: &Condition) -> bool {
        match (self, other) {
            (Condition::Create(utxo1, amount1), Condition::Create(utxo2, amount2)) => {
                utxo1 == utxo2 && amount1 != amount2
            }
            (Condition::ExtDeposit(addr1, amount1), Condition::ExtDeposit(addr2, amount2)) => {
                addr1 == addr2 && amount1 != amount2
            }
            (Condition::ExtWithdraw(addr1, amount1), Condition::ExtWithdraw(addr2, amount2)) => {
                addr1 == addr2 && amount1 != amount2
            }
            (
                Condition::ExtIntegration(adapter1, utxos1, amount1),
                Condition::ExtIntegration(adapter2, utxos2, amount2),
            ) => {
                let is_same_adapter = adapter1 == adapter2;

                // different adapters shouldn't overlap UTXOs
                if !is_same_adapter {
                    for utxo in utxos1.iter() {
                        if utxos2.contains(utxo) {
                            return true;
                        }
                    }
                    return false;
                }
                // Same adapter: check amounts and exact UTXO set match
                if amount1 != amount2 {
                    return true;
                }

                // Check if the UTXO sets are identical in size
                if utxos1.len() != utxos2.len() {
                    return true;
                }
                // Check all in utxos1 are in utxos2
                for utxo in utxos1.iter() {
                    if !utxos2.contains(utxo) {
                        return true;
                    }
                }
                // Check all in utxos2 are in utxos1 (ensures no extras)
                for utxo in utxos2.iter() {
                    if !utxos1.contains(utxo) {
                        return true;
                    }
                }
                false
            }

            _ => false,
        }
    }
}

//
#[derive(Clone)]
#[contracttype]
pub struct AuthRequirements(pub Map<SignerKey, Vec<Condition>>);

#[contracttype]
#[derive(Clone, Debug)]
pub struct Signatures(pub Map<SignerKey, (Signature, u32)>); // Signature with a valid_until_ledger

#[contracttype]
#[derive(Clone, Debug)]
pub enum SignerKey {
    P256(BytesN<65>),     // SEC1 uncompressed
    Ed25519(BytesN<32>),  // Ed25519 public key
    Provider(BytesN<32>), // Ed25519 public key of the provider account (Only native keys for now)
}

#[contracttype]
#[derive(Clone, Debug)]
pub enum Signature {
    P256(BytesN<64>),
    Ed25519(BytesN<64>),
    Secp256k1(BytesN<65>),
    BLS12_381(BytesN<96>),
}

#[derive(Clone)]
#[contracttype]
pub struct AuthPayload {
    pub conditions: Vec<Condition>,
    pub live_until_ledger: u32,
}

/// Constructs the payload for processing a bundle of UTXO operations.
///
/// The payload is built by concatenating, in this fixed order:
///  1. the `contract` address bytes (the ~56-byte strkey string as passed in),
///  2. the canonical XDR encoding of the ordered `Vec<Condition>` (`ToXdr`),
///  3. the `live_until_ledger` (4-byte little-endian `u32`).
///
/// The condition list is serialized with `ToXdr` — the same self-delimiting
/// on-wire representation that `equal_condition_sequence` compares against.
/// Because XDR length-prefixes vectors and tags every enum variant, the encoding
/// is **injective**: two distinct condition lists can never hash to the same
/// bytes. In particular an `ExtDeposit(X, a)` and an `ExtWithdraw(X, a)` over the
/// same address and amount now hash differently (the prior per-type bucket
/// concatenation made them byte-identical), and reordering the conditions changes
/// the hash — the digest binds the exact ordering the signer reviewed. The order
/// is therefore preserved as-is; conditions are never sorted or canonicalized.
///
/// The resulting byte stream is hashed using SHA-256 to produce a digest that is
/// used for verifying the signatures of the bundle.
///
pub fn hash_payload(e: &Env, auth_payload: &AuthPayload, contract: &Bytes) -> Hash<32> {
    let mut b = Bytes::new(&e);
    b.append(&contract);

    // Canonical, order-sensitive XDR of the condition list. XDR is self-delimiting
    // (length-prefixed vectors, variant-tagged enums), so distinct condition lists
    // always produce distinct bytes — closing the deposit/withdraw and reordering
    // collisions that the previous back-to-back bucket concatenation allowed.
    b.append(&auth_payload.conditions.clone().to_xdr(e));

    b.append(&Bytes::from_slice(
        &e,
        &auth_payload.live_until_ledger.to_le_bytes(),
    ));

    e.crypto().sha256(&b)
}

// Returns true if all BytesN<65> keys produced by key_fn are unique.
pub fn no_duplicate_keys<I, F>(e: &Env, iter: I, mut key_fn: F) -> bool
where
    I: IntoIterator,
    F: FnMut(I::Item) -> BytesN<65>,
{
    let mut seen: Map<BytesN<65>, bool> = Map::new(e);
    for item in iter {
        let k = key_fn(item);
        if seen.contains_key(k.clone()) {
            return false;
        }
        seen.set(k, true);
    }
    true
}

// Returns true if all Address keys produced by key_fn are unique.
pub fn no_duplicate_addresses<I, F>(e: &Env, iter: I, mut key_fn: F) -> bool
where
    I: IntoIterator,
    F: FnMut(I::Item) -> Address,
{
    let mut seen: Map<Address, bool> = Map::new(e);
    for item in iter {
        let k = key_fn(item);
        if seen.contains_key(k.clone()) {
            return false;
        }
        seen.set(k, true);
    }
    true
}

// Compares two Vec<Condition> for exact sequence equality using canonical XDR bytes.
pub fn equal_condition_sequence(e: &Env, a: &Vec<Condition>, b: &Vec<Condition>) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut it_b = b.iter();
    for ca in a.iter() {
        match it_b.next() {
            Some(cb) => {
                // Compare on-wire representation to avoid needing Eq on Condition.
                if ca.to_xdr(&e) != cb.to_xdr(&e) {
                    return false;
                }
            }
            None => return false,
        }
    }
    true
}

pub fn condition_does_not_conflict_with_set(
    condition: &Condition,
    condition_set: &Vec<Condition>,
) -> bool {
    for cond in condition_set.iter() {
        if cond.conflicts_with(condition) {
            return false;
        }
    }
    true
}

pub fn has_no_conflicting_conditions_in_sets(
    conditions_a: &Vec<Condition>,
    conditions_b: &Vec<Condition>,
) -> bool {
    for cond in conditions_a.iter() {
        if !condition_does_not_conflict_with_set(&cond, conditions_b) {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod test;
