#![no_std]
use soroban_sdk::{
    auth::Context,
    contracterror, contracttrait, contracttype,
    crypto::Hash,
    xdr::{self, FromXdr, ToXdr},
    Address, Bytes, BytesN, Env, FromVal, Map, TryFromVal, TryIntoVal, Vec,
};
use stellar_strkey::{ed25519, Strkey};

use moonlight_helpers::parser::address_from_ed25519_pk_bytes;

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

#[derive(Clone)]
#[contracttype]
pub struct AuthPayload {
    pub contract: Address,
    pub conditions: Vec<Condition>,
    pub live_until_ledger: u32,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct Signatures(pub Map<SignerKey, (Signature, u32)>); // Signature with a valid_until_ledger

#[contracttype]
#[derive(Clone, Debug)]
pub enum SignerKey {
    P256(BytesN<65>), // SEC1 uncompressed
    // Ed25519(BytesN<32>),   // Ed25519 public key
    // Secp256k1(BytesN<65>), // Secp256k1 public key
    // BLS12_381(BytesN<48>), // BLS12-381 public key (not implemented)
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
    SignatureExpired = 13,
    ProviderThresholdNotMet = 14,
}

fn verify_p256_signature(
    e: &Env,
    public_key: &BytesN<65>,
    signature: &BytesN<64>,
    payload_hash: &Hash<32>,
) -> Result<(), Error> {
    e.crypto()
        .secp256r1_verify(public_key, payload_hash, signature);

    Ok(())
}

fn verify_ed25519_signature(
    e: &Env,
    public_key: &BytesN<32>,
    signature: &BytesN<64>,
    payload_hash: &Hash<32>,
) -> Result<(), Error> {
    e.crypto().ed25519_verify(
        public_key,
        &Bytes::from_array(&e, &payload_hash.to_array()),
        signature,
    );

    Ok(())
}

pub fn verify_signature(
    e: &Env,
    signer: &SignerKey,
    signature: &Signature,
    payload_hash: &Hash<32>,
) -> Result<(), Error> {
    match (signer, signature) {
        (SignerKey::P256(pk), Signature::P256(sig)) => {
            verify_p256_signature(e, pk, sig, payload_hash)
        }
        (SignerKey::Provider(pk), Signature::Ed25519(sig)) => {
            verify_ed25519_signature(e, pk, sig, payload_hash)
        }

        // (SignerKey::Secp256k1(_pk), Signature::Secp256k1(_sig)) => {
        //     Err(Error::UnsupportedSignatureFormat)
        // }
        // (SignerKey::BLS12_381(_pk), Signature::BLS12_381(_sig)) => {
        //     Err(Error::UnsupportedSignatureFormat)
        // }
        _ => Err(Error::InvalidSignatureFormat),
    }
}

/// Constructs the payload for processing a bundle of UTXO operations.
///
/// The payload is built by concatenating in order:
///  - The contract address (32 bytes),
///  - The literal "CREATE" (6 bytes), followed by all `create` conditions,
///  - The literal "DEPOSIT" (8 bytes), followed by all `withdraw` conditions,
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
    let mut b_deposit = Bytes::new(&e);
    b_deposit.append(&Bytes::from_slice(&e, b"DEPOSIT"));
    let mut b_withdraw = Bytes::new(&e);
    b_withdraw.append(&Bytes::from_slice(&e, b"WITHDRAW"));
    let mut b_integrate = Bytes::new(&e);
    b_integrate.append(&Bytes::from_slice(&e, b"INTEGRATE"));

    for cond in auth_payload.conditions.iter() {
        match cond {
            Condition::Create(utxo, amount) => {
                b_create.append(&Bytes::from_slice(&e, utxo.to_array().as_ref()));
                b_create.append(&Bytes::from_slice(&e, &amount.to_le_bytes()));
            }
            Condition::ExtDeposit(addr, amount) => {
                b_deposit.append(&addr.to_xdr(&e));
                b_deposit.append(&Bytes::from_slice(&e, &amount.to_le_bytes()));
            }
            Condition::ExtWithdraw(addr, amount) => {
                b_withdraw.append(&addr.to_xdr(&e));
                b_withdraw.append(&Bytes::from_slice(&e, &amount.to_le_bytes()));
            }
            Condition::ExtIntegration(adapter, utxos, amount) => {
                b_integrate.append(&adapter.to_xdr(&e));
                for utxo in utxos.iter() {
                    b_integrate.append(&Bytes::from_slice(&e, utxo.to_array().as_ref()));
                }
                b_integrate.append(&Bytes::from_slice(&e, &amount.to_le_bytes()));
            }
        }
    }
    b.append(&b_create);
    b.append(&b_deposit);
    b.append(&b_withdraw);
    b.append(&b_integrate);

    b.append(&Bytes::from_slice(
        &e,
        &auth_payload.live_until_ledger.to_le_bytes(),
    ));

    e.crypto().sha256(&b)
}

#[contracttrait]
pub trait UtxoAuthorizable {
    #[internal]
    fn handle_utxo_auth(
        e: &Env,
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
                let v_req = cc.args.get(0).ok_or(Error::MissingArg)?;
                let reqs: AuthRequirements = v_req.try_into_val(e).map_err(|_| Error::BadArg)?;

                let inner: Map<SignerKey, Vec<Condition>> = reqs.0;

                let caller_contract = cc.contract;

                for signer in inner.keys().iter() {
                    let conds: Vec<Condition> = inner.get(signer.clone()).unwrap(); // or handle Option
                                                                                    // Use the signer key (signer) and its conditions (conds)

                    if conds.is_empty() {
                        return Err(Error::NoConditions);
                    }

                    match signer.clone() {
                        SignerKey::P256(signer_pk) => {
                            for existed in seen_pks.iter() {
                                if existed == signer_pk {
                                    return Err(Error::Duplicate);
                                }
                            }
                            seen_pks.push_back(signer_pk.clone());
                            required += 1;

                            // Lookup signature by key.

                            let (sig_variant, valid_until_ledger) =
                                sig_map.get(signer.clone()).ok_or(Error::MissingSignature)?;

                            if valid_until_ledger < e.ledger().sequence() {
                                return Err(Error::SignatureExpired);
                            }

                            let auth_payload = AuthPayload {
                                contract: caller_contract.clone(),
                                conditions: conds,
                                live_until_ledger: valid_until_ledger,
                            };

                            let msg = hash_payload(&e, &auth_payload);

                            verify_signature(&e, &signer, &sig_variant, &msg)?;
                        }
                        _ => {
                            // return Err(Error::UnsupportedSigner);
                            // TODO: Review as we'll have the provider auth here
                        }
                    }
                }
            } else {
                // Reject non-contract contexts
                return Err(Error::UnexpectedContext);
            }
        }

        // Fail if extra signatures were supplied (map has more than required).
        if sig_map.len() != required {
            // return Err(Error::ExtraSignature);
            // TODO: Review as we'll have the provider auth here
        }

        Ok(())
    }
}

#[derive(Clone)]
#[contracttype]
pub enum ProviderDataKey {
    AuthorizedProvider(Address),
}

#[contracttrait]
pub trait ProviderAuthorizable {
    /// Checks if the given address is a registered provider.
    ///
    /// Returns `true` if the provider is registered, `false` otherwise.
    ///
    fn is_provider(e: &Env, provider: Address) -> bool {
        e.storage()
            .instance()
            .get::<_, ()>(&ProviderDataKey::AuthorizedProvider(provider))
            .is_some()
    }

    /// Registers a new provider.
    ///
    /// ### Panics
    /// - Panics if the provider is already registered.
    #[internal]
    fn register_provider(e: &Env, provider: Address) {
        assert!(
            !Self::is_provider(&e, provider.clone()),
            "Provider already registered"
        );

        e.storage()
            .instance()
            .set(&ProviderDataKey::AuthorizedProvider(provider), &());
    }

    /// Deregisters a provider.
    ///
    /// ### Panics
    /// - Panics if the provider is not registered.
    #[internal]
    fn deregister_provider(e: &Env, provider: Address) {
        assert!(
            Self::is_provider(&e, provider.clone()),
            "Provider not registered"
        );

        e.storage()
            .instance()
            .remove(&ProviderDataKey::AuthorizedProvider(provider));
    }

    /// Requires that the given provider is registered
    ///  and that the transaction is authorized by the provider.
    ///
    /// ### Panics
    /// - Panics if the provider is not registered.
    /// - Panics if the transaction is not authorized by the provider.
    #[internal]
    fn require_provider(e: &Env, payload: Hash<32>, signatures: Signatures) -> Result<(), Error> {
        let sig_map = signatures.0;

        let mut provider_quorum = 0;
        const PROVIDER_THRESHOLD: u32 = 1; // For now we require only one exact provider signature

        for signer in sig_map.keys().iter() {
            if let SignerKey::Provider(pk32) = signer.clone() {
                let provider_addr: Address = address_from_ed25519_pk_bytes(&e, &pk32);

                assert!(
                    Self::is_provider(&e, provider_addr.clone()),
                    "Provider not registered"
                );
                let (sig_variant, valid_until_ledger) = sig_map
                    .get(signer.clone())
                    .ok_or(Error::MissingSignature)
                    .unwrap();

                if valid_until_ledger < e.ledger().sequence() {
                    return Err(Error::SignatureExpired);
                }

                verify_signature(&e, &signer, &sig_variant, &payload)?;

                provider_quorum += 1;
            }
        }

        if (provider_quorum < PROVIDER_THRESHOLD) {
            return Err(Error::ProviderThresholdNotMet);
        }

        Ok(())
    }
}
