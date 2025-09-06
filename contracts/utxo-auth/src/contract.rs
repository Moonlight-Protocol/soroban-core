// SignatureType {
//     Ed25519,   // Native Stellar keys
//     Secp256k1, // (secp256k1) used in Ethereum
//     P256,      // (secp256r1) passkeys used in webauthn
//     BLS12_381, // BLS signatures
// }

use admin_sep::{Administratable, Upgradable};
use soroban_sdk::{
    auth::{Context, CustomAccountInterface},
    contract, contracterror, contractimpl, contracttype,
    crypto::Hash,
    xdr::ToXdr,
    Address, Bytes, BytesN, Env, Map, Symbol, TryIntoVal, Vec,
};

#[contract]
pub struct UTXOAuthContract;

#[contractimpl]
impl Administratable for UTXOAuthContract {}

#[contractimpl]
impl Upgradable for UTXOAuthContract {}

// pub struct SignatureBundle {
//     pub signatures: Vec<UTXOSignature>,
// }

#[contracttype]
#[derive(Clone, Debug)]
pub struct Signatures(pub Map<SignerKey, Signature>);

#[contracttype]
#[derive(Clone, Debug)]
pub enum SignerKey {
    P256(BytesN<65>), // SEC1 uncompressed
}

#[contracttype]
#[derive(Clone, Debug)]
pub enum Signature {
    P256(BytesN<64>), // r||s
}

#[contracterror]
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[repr(u32)]
pub enum Error {
    Duplicate = 1,
    MissingArg = 2,
    BadArg = 3,
    UnexpectedVariant = 4,
    MissingSignature = 5,
    ExtraSignature = 6,
    Test = 999, // for debugging
}

#[derive(Clone)]
#[contracttype]
pub struct Bundle {
    pub spend: Vec<BytesN<65>>,
    pub create: Vec<(BytesN<65>, i128)>,
}

#[contracttype]
#[derive(Clone)]
pub struct UtxoSpendAuth {
    pub pk: BytesN<65>, // SEC1 uncompressed
    pub bundle: Bundle, // per-UTXO bundle
    pub action: Symbol, // e.g. "TRANSFER", "DELEGATED_TRANSFER", "CUSTOM"
}

#[contracttype]
#[derive(Clone)]
pub enum AuthRequest {
    Spend(UtxoSpendAuth),
}

#[contractimpl]
impl UTXOAuthContract {
    pub fn __constructor(env: &Env, admin: &Address) {
        Self::set_admin(env, admin);
    }
}
#[contractimpl]
impl CustomAccountInterface for UTXOAuthContract {
    type Error = Error;
    type Signature = Signatures;

    fn __check_auth(
        e: Env,
        _payload: Hash<32>,     // intentionally unused (decoupled per-UTXO)
        signatures: Signatures, // provided by tx submitter in Authorization entry
        contexts: Vec<Context>, // all require_auth_for_args sites
    ) -> Result<(), Error> {
        // Map of provided signatures.
        let sig_map = signatures.0;

        // Track seen public keys to forbid duplicates in the same authorization set.
        let mut seen_pks: Vec<BytesN<65>> = Vec::new(&e);

        // Count how many Spend auths are required.
        let mut required = 0u32;

        for c in contexts.iter() {
            if let Context::Contract(cc) = c {
                let v = cc.args.get(0).ok_or(Error::MissingArg)?;
                let req: AuthRequest = v.try_into_val(&e).map_err(|_| Error::BadArg)?;

                if let AuthRequest::Spend(UtxoSpendAuth { pk, bundle, action }) = req {
                    // Reject duplicate pk.
                    for existed in seen_pks.iter() {
                        if existed == pk {
                            return Err(Error::Duplicate);
                        }
                    }
                    seen_pks.push_back(pk.clone());
                    required += 1;

                    // Build canonical message for this bundle.
                    let msg = bundle_payload(&e, bundle, &action);

                    // Lookup signature by key.
                    let key = SignerKey::P256(pk.clone());
                    let sig_variant = sig_map.get(key).ok_or(Error::MissingSignature)?;

                    match sig_variant {
                        Signature::P256(sig_bytes) => {
                            verify_secp256r1_signature(&e, &pk, &sig_bytes, &msg);
                        }
                    }
                } else {
                    return Err(Error::UnexpectedVariant);
                }
            } else {
                // Reject non-contract contexts (adjust if you prefer to ignore).
                return Err(Error::Test);
            }
        }

        // Fail if extra signatures were supplied (map has more than required).
        if sig_map.len() != required {
            return Err(Error::ExtraSignature);
        }

        Ok(())
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

/// Constructs the payload for processing a bundle of UTXO operations.
///
/// The payload is built by concatenating:
/// - The literal "BUNDLE" (6 bytes),
/// - The provided action string(Symbol) (e.g., "TRANSFER", "DELEGATED_TRANSFER", "CUSTOM"),
/// - For each UTXO in the `spend` list: its 65-byte public key,
/// - For each entry in the `create` list: the UTXO's 65-byte public key followed by its 8-byte little-endian amount.
///
/// The resulting byte stream is hashed using SHA-256 to produce a digest that is used for verifying the signatures of the bundle.
pub fn bundle_payload(e: &Env, bundle: Bundle, action: &Symbol) -> Hash<32> {
    let mut b = Bytes::new(&e);

    b.append(&Bytes::from_slice(&e, b"BUNDLE"));

    // Convert Symbol to bytes using its Val representation
    b.append(&action.to_xdr(&e));

    for utxo in bundle.spend.iter() {
        b.append(&Bytes::from_slice(&e, utxo.to_array().as_ref()));
    }

    for (utxo, amount) in bundle.create.iter() {
        b.append(&Bytes::from_slice(&e, utxo.to_array().as_ref()));
        b.append(&Bytes::from_slice(&e, &amount.to_le_bytes()));
    }

    e.crypto().sha256(&b)
}
