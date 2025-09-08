#![no_std]

use soroban_sdk::{contracterror, contracttype, crypto::Hash, BytesN, Env, Map};

#[contracttype]
#[derive(Clone, Debug)]
pub struct Signatures(pub Map<SignerKey, (Signature, u32)>);

#[contracttype]
#[derive(Clone, Debug)]
pub enum SignerKey {
    P256(BytesN<65>),      // SEC1 uncompressed
    Ed25519(BytesN<32>),   // Ed25519 public key
    Secp256k1(BytesN<65>), // Secp256k1 public key
    BLS12_381(BytesN<48>), // BLS12-381 public key (not implemented)
}

#[contracttype]
#[derive(Clone, Debug)]
pub enum Signature {
    P256(BytesN<64>), // r||s
    Ed25519(BytesN<64>),
    Secp256k1(BytesN<65>), // DER encoded
    BLS12_381(BytesN<96>), // BLS12-381 signature
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
        (SignerKey::Ed25519(_pk), Signature::Ed25519(_sig)) => {
            Err(Error::UnsupportedSignatureFormat)
        }
        (SignerKey::Secp256k1(_pk), Signature::Secp256k1(_sig)) => {
            Err(Error::UnsupportedSignatureFormat)
        }
        (SignerKey::BLS12_381(_pk), Signature::BLS12_381(_sig)) => {
            Err(Error::UnsupportedSignatureFormat)
        }
        _ => Err(Error::InvalidSignatureFormat),
    }
}

#[contracterror]
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[repr(u32)]
pub enum Error {
    InvalidSignatureFormat = 501,
    UnsupportedSignatureFormat = 502,
}
