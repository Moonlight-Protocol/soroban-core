use ed25519_dalek::{Keypair as NativeKeypair, Signer as NativeSigner};
use p256::{
    ecdsa::{
        signature::hazmat::PrehashSigner, Signature as P256Signature, SigningKey as P256SigningKey,
        VerifyingKey as P256VerifyingKey,
    },
    elliptic_curve::scalar::IsHigh,
};
use rand::rngs::OsRng;
use rand_core::OsRng as CoreOsRng;

use soroban_sdk::{contracttype, crypto::Hash, Address, Bytes, BytesN, Env, TryFromVal};
use stellar_strkey::{ed25519, Strkey};

#[derive(Clone)]
#[contracttype]
pub struct AccountEd25519Signature {
    public_key: BytesN<32>,
    signature: BytesN<64>,
}

pub struct Ed25519Account {
    pub public_key: BytesN<32>,
    pub keypair: NativeKeypair,
    pub address: Address,
}

impl Ed25519Account {
    pub fn from_keys(e: &Env, public: &[u8; 32], secret: &[u8; 32]) -> Ed25519Account {
        let public =
            ed25519_dalek::PublicKey::from_bytes(public).expect("32 bytes, valid ed25519 pk");
        let secret =
            ed25519_dalek::SecretKey::from_bytes(secret).expect("64 bytes, within curve order");

        let keypair = NativeKeypair {
            secret,
            public: public.try_into().expect("32 bytes, valid ed25519 pk"),
        };

        Self::from_keypair(e, keypair)
    }

    pub fn generate(e: &Env) -> Ed25519Account {
        let mut csprng = OsRng {};

        let keypair = NativeKeypair::generate(&mut csprng);

        Self::from_keypair(e, keypair)
    }

    pub fn from_keypair(e: &Env, keypair: NativeKeypair) -> Ed25519Account {
        let public_key_str =
            Strkey::PublicKeyEd25519(ed25519::PublicKey(keypair.public.to_bytes()));

        let address_bytes = Bytes::from_slice(&e, public_key_str.to_string().as_bytes());
        let address = Address::from_string_bytes(&address_bytes);

        let public_key = BytesN::<32>::from_array(&e, &keypair.public.to_bytes());

        Ed25519Account {
            public_key,
            keypair,
            address,
        }
    }

    pub fn sign(&self, e: &Env, msg: Hash<32>) -> BytesN<64> {
        let signed_payload = self.keypair.sign(msg.to_array().as_slice()).to_bytes();

        BytesN::from_array(&e, &signed_payload)
    }

    pub fn sign_for_transaction(&self, e: &Env, msg: Hash<32>) -> AccountEd25519Signature {
        let raw_signature = self.sign(e, msg);

        AccountEd25519Signature {
            public_key: BytesN::<32>::try_from_val(e, &self.keypair.public.to_bytes()).unwrap(),
            signature: BytesN::<64>::try_from_val(e, &raw_signature).unwrap(),
        }
    }
}

pub struct P256KeyPair {
    pub public_key: BytesN<65>,
    secret_key: P256SigningKey,
}

impl P256KeyPair {
    pub fn generate(env: &Env) -> P256KeyPair {
        let signing_key = P256SigningKey::random(&mut CoreOsRng);
        let verifying_key = signing_key.verifying_key();

        let public_key_bytes: [u8; 65] = verifying_key
            .to_encoded_point(false)
            .as_bytes()
            .try_into()
            .unwrap();

        let public_key = BytesN::<65>::from_array(env, &public_key_bytes);

        let test_verifying_key = P256VerifyingKey::from_sec1_bytes(&public_key_bytes);
        assert!(test_verifying_key.is_ok(), "Public key generation failed!");

        P256KeyPair {
            public_key,
            secret_key: signing_key,
        }
    }

    /// Sign a message hash and normalize the signature to low-S.
    pub fn sign(&self, msg: &Hash<32>) -> [u8; 64] {
        let mut signature: P256Signature = self.secret_key.sign_prehash(&msg.to_array()).unwrap();
        if bool::from(signature.s().is_high()) {
            signature = signature.normalize_s().unwrap();
        }

        // Convert signature to raw bytes (r || s format)
        let mut signature_bytes = [0u8; 64];
        signature_bytes[..32].copy_from_slice(&signature.r().to_bytes());
        signature_bytes[32..].copy_from_slice(&signature.s().to_bytes());

        signature_bytes
    }

    pub fn sign_with_key(secret_key: P256SigningKey, msg: &Hash<32>) -> [u8; 64] {
        let mut signature: P256Signature = secret_key.sign_prehash(&msg.to_array()).unwrap();
        if bool::from(signature.s().is_high()) {
            signature = signature.normalize_s().unwrap();
        }

        // Convert signature to raw bytes (r || s format)
        let mut signature_bytes = [0u8; 64];
        signature_bytes[..32].copy_from_slice(&signature.r().to_bytes());
        signature_bytes[32..].copy_from_slice(&signature.s().to_bytes());

        signature_bytes
    }
}
