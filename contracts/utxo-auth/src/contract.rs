use admin_sep::{Administratable, Upgradable};
use soroban_sdk::{
    auth::{Context, CustomAccountInterface},
    contract, contracterror, contractimpl, contracttype,
    crypto::Hash,
    xdr::ToXdr,
    Address, Bytes, BytesN, Env, Map, Symbol, TryIntoVal, Vec,
};

use crate::{
    payload::{hash_payload, AuthPayload},
    signature::{verify_signature, Signatures, SignerKey},
};

#[contract]
pub struct UTXOAuthContract;

#[contractimpl]
impl Administratable for UTXOAuthContract {}

#[contractimpl]
impl Upgradable for UTXOAuthContract {}

// #[derive(Clone)]
// #[contracttype]
// pub struct Bundle {
//     pub spend: Vec<BytesN<65>>,
//     pub create: Vec<(BytesN<65>, i128)>,
// }

// #[contracttype]
// #[derive(Clone)]
// pub struct UtxoSpendAuth {
//     pub pk: BytesN<65>, // SEC1 uncompressed
//     pub bundle: Bundle, // per-UTXO bundle
//     pub action: Symbol, // e.g. "TRANSFER", "DELEGATED_TRANSFER", "CUSTOM"
// }

// #[contracttype]
// #[derive(Clone)]
// pub enum AuthRequest {
//     Spend(UtxoSpendAuth),
// }

// Require Args
// 0: SignerKey
// 1: AuthPayload
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
                let v_signer = cc.args.get(0).ok_or(Error::MissingArg)?;
                let signer: SignerKey = v_signer.try_into_val(&e).map_err(|_| Error::BadArg)?;

                let v_auth_payload = cc.args.get(1).ok_or(Error::MissingArg)?;
                let auth_payload: AuthPayload =
                    v_auth_payload.try_into_val(&e).map_err(|_| Error::BadArg)?;

                if auth_payload.contract != cc.contract {
                    return Err(Error::MismatchedContract);
                }
                if auth_payload.conditions.is_empty() {
                    return Err(Error::NoConditions);
                }

                match signer.clone() {
                    SignerKey::P256(pk) => {
                        for existed in seen_pks.iter() {
                            if existed == pk {
                                return Err(Error::Duplicate);
                            }
                        }
                        seen_pks.push_back(pk.clone());
                        required += 1;

                        let msg = hash_payload(&e, &auth_payload);

                        // Lookup signature by key.
                        let key = SignerKey::P256(pk.clone());
                        let sig_variant = sig_map.get(key).ok_or(Error::MissingSignature)?;

                        verify_signature(&e, &signer, &sig_variant, &msg)?;
                    }
                    _ => {
                        return Err(Error::UnsupportedSigner);
                    }
                }
            } else {
                // Reject non-contract contexts
                return Err(Error::UnexpectedContext);
            }
        }

        // Fail if extra signatures were supplied (map has more than required).
        if sig_map.len() != required {
            return Err(Error::ExtraSignature);
        }

        Ok(())
    }
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
    InvalidSignatureFormat = 7,
    UnsupportedSignatureFormat = 8,
    MismatchedContract = 9,
    UnsupportedSigner = 10,
    NoConditions = 11,
    UnexpectedContext = 12,
    Test = 999, // for debugging
}
