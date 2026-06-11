pub use moonlight_errors::Error;
use moonlight_helpers::parser::address_from_ed25519_pk_bytes;
use moonlight_primitives::{
    hash_payload, AuthPayload, AuthRequirements, Condition, Signature, Signatures, SignerKey,
};
use soroban_sdk::{
    assert_with_error, auth::Context, contracttype, crypto::Hash, Address, Bytes, BytesN, Env, Map,
    TryIntoVal, Vec,
};

/// Verify a secp256r1 (P-256) signature.
///
/// # Safety / invariant (MOON-04)
/// `secp256r1_verify` returns `()` and **panics (traps the transaction) on an invalid signature**;
/// it never returns an error to inspect. This wrapper therefore returns `Ok(())` unconditionally
/// and relies entirely on that panic-on-failure semantic of soroban-sdk 25.3.x — there is no
/// success/failure value to branch on. If a future SDK changes these primitives to RETURN a
/// result instead of panicking, this wrapper would silently accept invalid signatures and MUST be
/// rewritten to check the returned value. The `signature_verification_*` regression tests lock the
/// current behavior; re-validate them on every SDK upgrade.
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

/// Verify an Ed25519 signature.
///
/// # Safety / invariant (MOON-04)
/// See [`verify_p256_signature`]: `ed25519_verify` panics on an invalid signature and returns no
/// inspectable result, so this wrapper depends on that panic-on-failure semantic. Re-validate on
/// every SDK upgrade.
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
        _ => Err(Error::InvalidSignatureFormat),
    }
}

pub trait UtxoAuthorizable {
    fn handle_utxo_auth(
        e: &Env,
        signatures: Signatures, // provided by tx submitter in Authorization entry
        contexts: Vec<Context>, // all require_auth_for_args sites
    ) -> Result<(), Error> {
        for c in contexts.iter() {
            if let Context::Contract(cc) = c {
                let sig_map = signatures.0.clone();

                if cc.args.len() < 1 {
                    // MOON-03: a context with no auth-requirements arg carries no UTXO
                    // requirements, but it must only skip THIS context — never short-circuit the
                    // whole check. A `return Ok(())` here would let an empty-args context that
                    // precedes a spend-bearing context bypass the latter's P256 verification.
                    continue;
                }

                let v_req = cc.args.get(0).ok_or(Error::BadArg)?;
                let reqs: AuthRequirements = v_req.try_into_val(e).map_err(|_| Error::BadArg)?;

                let inner: Map<SignerKey, Vec<Condition>> = reqs.0;

                let caller_contract = cc.contract;
                let caller_contract_bytes: Bytes = caller_contract.clone().to_string().to_bytes();
                for signer in inner.keys().iter() {
                    let conds: Vec<Condition> = inner.get(signer.clone()).unwrap(); // or handle Option
                                                                                    // Use the signer key (signer) and its conditions (conds)

                    if conds.is_empty() {
                        return Err(Error::NoConditions);
                    }

                    match signer.clone() {
                        SignerKey::P256(_signer_pk) => {
                            // Lookup signature by key.

                            let (sig_variant, valid_until_ledger) =
                                sig_map.get(signer.clone()).ok_or(Error::MissingSignature)?;

                            if valid_until_ledger < e.ledger().sequence() {
                                return Err(Error::SignatureExpired);
                            }

                            let auth_payload = AuthPayload {
                                conditions: conds,
                                live_until_ledger: valid_until_ledger,
                            };

                            let msg =
                                hash_payload(&e, &auth_payload, &caller_contract_bytes.clone());

                            verify_signature(&e, &signer, &sig_variant, &msg)?;
                        }
                        _ => {
                            //Do nothing as we might have the provider signature along these
                            continue;
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
    fn register_provider(e: &Env, provider: Address) {
        assert_with_error!(
            e,
            !Self::is_provider(&e, provider.clone()),
            Error::ProviderAlreadyRegistered
        );

        e.storage()
            .instance()
            .set(&ProviderDataKey::AuthorizedProvider(provider), &());
    }

    /// Deregisters a provider.
    ///
    /// ### Panics
    /// - Panics if the provider is not registered.
    fn deregister_provider(e: &Env, provider: Address) {
        assert_with_error!(
            e,
            Self::is_provider(&e, provider.clone()),
            Error::ProviderNotRegistered
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
    fn require_provider(e: &Env, payload: Hash<32>, signatures: Signatures) -> Result<(), Error> {
        let sig_map = signatures.0;

        let mut provider_quorum = 0;
        const PROVIDER_THRESHOLD: u32 = 1; // For now we require only one exact provider signature

        for signer in sig_map.keys().iter() {
            if let SignerKey::Provider(pk32) = signer.clone() {
                let provider_addr: Address = address_from_ed25519_pk_bytes(&e, &pk32);

                assert_with_error!(
                    e,
                    Self::is_provider(&e, provider_addr.clone()),
                    Error::ProviderNotRegistered
                );
                let (sig_variant, valid_until_ledger) =
                    sig_map.get(signer.clone()).ok_or(Error::MissingSignature)?;

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
