use soroban_sdk::{contracttype, crypto::Hash, xdr::ToXdr, Address, Bytes, BytesN, Env, Vec};

#[derive(Clone)]
#[contracttype]
pub struct AuthPayload {
    pub contract: Address,
    pub conditions: Vec<SpendingCondition>,
}

#[derive(Clone)]
#[contracttype]
pub enum SpendingCondition {
    Create(BytesN<65>, i128),                    // Spend to create new UTXOs
    Withdraw(Address, i128),                     // Spend to withdraw to an account
    Integration(Address, Vec<BytesN<65>>, i128), // contract id of the adapter, the utxo(s) for withdrawal, the amount to deposit
}

/// Constructs the payload for processing a bundle of UTXO operations.
///
/// The payload is built by concatenating in order:
///  - The contract address (32 bytes),
///  - The literal "CREATE" (6 bytes), followed by all `create` conditions,
///  - The literal "WITHDRAW" (8 bytes), followed by all `withdraw` conditions,
///  - The literal "INTEGRATE" (9 bytes), followed by all `integration` conditions.
///
/// The resulting byte stream is hashed using SHA-256 to produce a digest that is
/// used for verifying the signatures of the bundle.
///
/// For consistency, all integer amounts are encoded as little-endian 8-byte sequences.
/// UTXO identifiers are represented as their raw byte arrays. Also it is suggested to sort
/// the conditions in the same ordering as they are defined in the original bundle  to ensure
/// deterministic payloads.
///
pub fn hash_payload(e: &Env, auth_payload: &AuthPayload) -> Hash<32> {
    let mut b = Bytes::new(&e);
    b.append(&auth_payload.contract.clone().to_xdr(&e));

    let mut b_create = Bytes::new(&e);
    b_create.append(&Bytes::from_slice(&e, b"CREATE"));
    let mut b_withdraw = Bytes::new(&e);
    b_withdraw.append(&Bytes::from_slice(&e, b"WITHDRAW"));
    let mut b_integrate = Bytes::new(&e);
    b_integrate.append(&Bytes::from_slice(&e, b"INTEGRATE"));

    for cond in auth_payload.conditions.iter() {
        match cond {
            SpendingCondition::Create(utxo, amount) => {
                b_create.append(&Bytes::from_slice(&e, utxo.to_array().as_ref()));
                b_create.append(&Bytes::from_slice(&e, &amount.to_le_bytes()));
            }
            SpendingCondition::Withdraw(addr, amount) => {
                b_withdraw.append(&addr.to_xdr(&e));
                b_withdraw.append(&Bytes::from_slice(&e, &amount.to_le_bytes()));
            }
            SpendingCondition::Integration(adapter, utxos, amount) => {
                b_integrate.append(&adapter.to_xdr(&e));
                for utxo in utxos.iter() {
                    b_integrate.append(&Bytes::from_slice(&e, utxo.to_array().as_ref()));
                }
                b_integrate.append(&Bytes::from_slice(&e, &amount.to_le_bytes()));
            }
        }
    }
    b.append(&b_create);
    b.append(&b_withdraw);
    b.append(&b_integrate);

    e.crypto().sha256(&b)
}
