use moonlight_helpers::parser::address_from_ed25519_pk_bytes;
use moonlight_primitives::{
    hash_payload, AuthPayload, AuthRequirements, Condition, Signature, Signatures, SignerKey,
};
use soroban_sdk::{
    auth::Context, contracterror, contracttrait, contracttype, crypto::Hash, Address, Bytes,
    BytesN, Env, Map, TryIntoVal, Vec,
};

#[contracterror]
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[repr(u32)]
pub enum Error {
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
        (SignerKey::Ed25519(pk), Signature::Ed25519(sig)) => {
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

#[contracttrait]
pub trait UtxoAuthorizable {
    #[internal]
    fn handle_utxo_auth(
        e: &Env,
        signatures: Signatures, // provided by tx submitter in Authorization entry
        contexts: Vec<Context>, // all require_auth_for_args sites
    ) -> Result<(), Error> {
        for c in contexts.iter() {
            if let Context::Contract(cc) = c {
                let sig_map = signatures.0.clone();

                if cc.args.len() < 1 {
                    return Ok(()); // No auth requirements, skip
                }

                let v_req = cc.args.get(0).ok_or(Error::BadArg)?;
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
                            //Do nothing as we might have the provider signature along these
                        }
                    }
                }
            } else {
                // Reject non-contract contexts
                return Err(Error::UnexpectedContext);
            }
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

        if provider_quorum < PROVIDER_THRESHOLD {
            return Err(Error::ProviderThresholdNotMet);
        }

        Ok(())
    }
}
