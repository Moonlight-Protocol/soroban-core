use crate::core::{Bundle, OperationBundle, UtxoHandlerTrait};
use moonlight_auth::{AuthRequirements, Condition, SignerKey};
use soroban_sdk::{
    contract, contractimpl, contracttrait, crypto::Hash, vec, Address, BytesN, Env, Map, Vec,
};
// use elliptic_curve::scalar::IsHigh;
// use p256::ecdsa::{signature::hazmat::PrehashSigner, Signature, SigningKey, VerifyingKey};
// use rand_core::OsRng;

// use crate::core::{
//     burn, burn_batch, delegated_transfer, mint, mint_batch, transfer, transfer_burn_leftover,
//     utxo_balance, Bundle, BurnRequest, MintRequest,
// };
// // #[contracttrait]
// // pub trait UTXOHandler {
// //     // fn mint(e: Env, amount: i128, utxo: BytesN<65>);
// //     // fn mint_batch(e: Env, requests: Vec<MintRequest>);
// //     fn utxo_balance(e: Env, utxo: BytesN<65>) -> i128;
// //     // fn burn(e: Env, utxo: BytesN<65>, signature: BytesN<64>);
// //     // fn burn_batch(e: Env, requests: Vec<BurnRequest>);
// //     // fn transfer(e: Env, bundles: Vec<Bundle>);
// //     // fn delegated_transfer(e: Env, bundles: Vec<Bundle>, delegate_utxo: BytesN<65>);
// //     // fn transfer_custom_leftover(e: Env, bundles: Vec<Bundle>) -> i128;
// // }

// pub struct KeyPair {
//     pub public_key: BytesN<65>,
//     pub secret_key: SigningKey,
// }

// /// Generate a new secp256r1 (P-256) key pair compatible with Soroban.
// pub fn generate_utxo_keypair(env: &Env) -> KeyPair {
//     let signing_key = SigningKey::random(&mut OsRng);
//     let verifying_key = signing_key.verifying_key();

//     let public_key_bytes: [u8; 65] = verifying_key
//         .to_encoded_point(false)
//         .as_bytes()
//         .try_into()
//         .unwrap();

//     let public_key = BytesN::<65>::from_array(env, &public_key_bytes);

//     let test_verifying_key = VerifyingKey::from_sec1_bytes(&public_key_bytes);
//     assert!(test_verifying_key.is_ok(), "Public key generation failed!");

//     KeyPair {
//         public_key,
//         secret_key: signing_key,
//     }
// }

// /// Sign a message hash and normalize the signature to low-S.
// pub fn sign_hash(secret_key: &SigningKey, hash: &Hash<32>) -> [u8; 64] {
//     let mut signature: Signature = secret_key.sign_prehash(&hash.to_array()).unwrap();
//     if bool::from(signature.s().is_high()) {
//         signature = signature.normalize_s().unwrap();
//     }

//     // Convert signature to raw bytes (r || s format)
//     let mut signature_bytes = [0u8; 64];
//     signature_bytes[..32].copy_from_slice(&signature.r().to_bytes());
//     signature_bytes[32..].copy_from_slice(&signature.s().to_bytes());

//     signature_bytes
// }

#[contract]
pub struct TestContract;

#[contractimpl]
impl UtxoHandlerTrait for TestContract {
    fn transact(e: Env, op: OperationBundle) -> i128 {
        let mut spend: Vec<BytesN<65>> = vec![&e];
        let mut create: Vec<(BytesN<65>, i128)> = vec![&e];

        for (spend_utxo, _conditions) in op.spend.iter() {
            spend.push_back(spend_utxo.clone());
        }
        for create_utxo in op.create.iter() {
            create.push_back(create_utxo.clone());
        }

        let req = calculate_auth_requirements(&e, op);

        let bundle: Bundle = Bundle {
            spend: spend,
            create: create,
            req,
        };

        Self::process_bundle(e, bundle, 0, 0)
    }
}

#[contractimpl]
impl TestContract {
    pub fn __constructor(e: Env, utxo_auth: Address) {
        Self::set_utxo_auth(&e, &utxo_auth);
    }

    pub fn mint_unchecked(e: Env, utxos: Vec<(BytesN<65>, i128)>) {
        for (utxo, amount) in utxos {
            Self::create(&e, amount, utxo);
        }
    }
}

pub fn create_contract(e: &Env, auth: Address) -> TestContractClient {
    let contract_id = e.register(TestContract, TestContractArgs::__constructor(&auth));
    let contract = TestContractClient::new(e, &contract_id);
    // Initialize contract if needed
    contract
}

pub fn calculate_auth_requirements(e: &Env, op: OperationBundle) -> AuthRequirements {
    let mut map_req: Map<SignerKey, Vec<Condition>> = Map::new(&e);

    for (spend_utxo, conditions) in op.spend.iter() {
        map_req.set(SignerKey::P256(spend_utxo.clone()), conditions.clone());
    }

    AuthRequirements(map_req)
}
