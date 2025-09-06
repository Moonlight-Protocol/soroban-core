use soroban_sdk::{contract, contractimpl, contracttrait, crypto::Hash, BytesN, Env, Vec};

use elliptic_curve::scalar::IsHigh;
use p256::ecdsa::{signature::hazmat::PrehashSigner, Signature, SigningKey, VerifyingKey};
use rand_core::OsRng;

use crate::core::{
    burn, burn_batch, delegated_transfer, mint, mint_batch, transfer, transfer_burn_leftover,
    utxo_balance, Bundle, BurnRequest, MintRequest,
};
#[contracttrait]
pub trait UTXOHandler {
    fn mint(e: Env, amount: i128, utxo: BytesN<65>);
    fn mint_batch(e: Env, requests: Vec<MintRequest>);
    fn utxo_balance(e: Env, utxo: BytesN<65>) -> i128;
    fn burn(e: Env, utxo: BytesN<65>, signature: BytesN<64>);
    fn burn_batch(e: Env, requests: Vec<BurnRequest>);
    fn transfer(e: Env, bundles: Vec<Bundle>);
    fn delegated_transfer(e: Env, bundles: Vec<Bundle>, delegate_utxo: BytesN<65>);
    fn transfer_custom_leftover(e: Env, bundles: Vec<Bundle>) -> i128;
}

pub struct KeyPair {
    pub public_key: BytesN<65>,
    pub secret_key: SigningKey,
}

/// Generate a new secp256r1 (P-256) key pair compatible with Soroban.
pub fn generate_utxo_keypair(env: &Env) -> KeyPair {
    let signing_key = SigningKey::random(&mut OsRng);
    let verifying_key = signing_key.verifying_key();

    let public_key_bytes: [u8; 65] = verifying_key
        .to_encoded_point(false)
        .as_bytes()
        .try_into()
        .unwrap();

    let public_key = BytesN::<65>::from_array(env, &public_key_bytes);

    let test_verifying_key = VerifyingKey::from_sec1_bytes(&public_key_bytes);
    assert!(test_verifying_key.is_ok(), "Public key generation failed!");

    KeyPair {
        public_key,
        secret_key: signing_key,
    }
}

/// Sign a message hash and normalize the signature to low-S.
pub fn sign_hash(secret_key: &SigningKey, hash: &Hash<32>) -> [u8; 64] {
    let mut signature: Signature = secret_key.sign_prehash(&hash.to_array()).unwrap();
    if bool::from(signature.s().is_high()) {
        signature = signature.normalize_s().unwrap();
    }

    // Convert signature to raw bytes (r || s format)
    let mut signature_bytes = [0u8; 64];
    signature_bytes[..32].copy_from_slice(&signature.r().to_bytes());
    signature_bytes[32..].copy_from_slice(&signature.s().to_bytes());

    signature_bytes
}

#[contract]
pub struct TestContract;

#[contractimpl]
impl TestContract {
    pub fn __constructor(_e: Env) {}
}

#[contractimpl]
impl UTXOHandler for TestContract {
    fn mint(e: Env, amount: i128, utxo: BytesN<65>) {
        mint(&e, amount, utxo);
    }

    fn mint_batch(e: Env, requests: Vec<MintRequest>) {
        mint_batch(&e, requests);
    }

    fn utxo_balance(e: Env, utxo: BytesN<65>) -> i128 {
        utxo_balance(e, utxo)
    }

    fn burn(e: Env, utxo: BytesN<65>, signature: BytesN<64>) {
        burn(&e, utxo, signature);
    }

    fn burn_batch(e: Env, requests: Vec<BurnRequest>) {
        burn_batch(&e, requests);
    }

    fn transfer(e: Env, bundles: Vec<Bundle>) {
        transfer(&e, bundles);
    }

    fn delegated_transfer(e: Env, bundles: Vec<Bundle>, delegate_utxo: BytesN<65>) {
        delegated_transfer(&e, bundles, delegate_utxo);
    }

    fn transfer_custom_leftover(e: Env, bundles: Vec<Bundle>) -> i128 {
        transfer_burn_leftover(&e, bundles, "CUSTOM")
    }
}

pub fn create_contract(e: &Env) -> TestContractClient {
    let contract_id = e.register(TestContract, TestContractArgs::__constructor());
    let contract = TestContractClient::new(e, &contract_id);
    // Initialize contract if needed
    contract
}
