#![cfg(test)]
use soroban_sdk::{
    symbol_short, vec,
    xdr::{self, SorobanAddressCredentials, ToXdr, VecM},
    Address, BytesN, Env, Map, TryIntoVal, Val, Vec,
};
extern crate std;

use utxo::tests::helpers::{generate_utxo_keypair, sign_hash};

use crate::{
    contract::{
        bundle_payload, AuthRequest, Bundle, Signature, Signatures, SignerKey,
        UTXOAuthContractClient, UtxoSpendAuth,
    },
    tests::helpers::{create_contracts, TestContractClient},
};

#[test]
fn test_auth_bundle_success() {
    let e = Env::default();

    let (_admin, auth, utxo_mock): (Address, UTXOAuthContractClient, TestContractClient) =
        create_contracts(&e);

    assert_eq!(utxo_mock.auth_address(), auth.address);

    let utxo_keypair_a = generate_utxo_keypair(&e);
    let utxo_keypair_b = generate_utxo_keypair(&e);

    let mut bundle_a = Bundle {
        spend: vec![&e, utxo_keypair_a.public_key.clone()],
        create: vec![&e, (utxo_keypair_b.public_key.clone(), 250)],
    };

    let hash_a = bundle_payload(&e, bundle_a.clone(), &symbol_short!("TRANSFER"));

    let signature_a: [u8; 64] = sign_hash(&utxo_keypair_a.secret_key, &hash_a);

    let signature_bytes_a = BytesN::<64>::from_array(&e, &signature_a);

    let invocation_args = vec![&e, bundle_a.clone()];

    let nonce = 0;
    let signature_expiration_ledger = e.ledger().sequence() + 100;

    let mut sign_map = Map::new(&e);
    sign_map.set(
        SignerKey::P256(utxo_keypair_a.public_key.clone()),
        Signature::P256(signature_bytes_a.clone()),
    );

    let signatures: Signatures = Signatures(sign_map);
    // {
    //     pk: utxo_keypair_a.public_key.clone(),
    //     sig: signature_bytes_a,
    // };

    let spend_auth: UtxoSpendAuth = UtxoSpendAuth {
        pk: utxo_keypair_a.public_key.clone(),
        bundle: bundle_a.clone(),
        action: symbol_short!("TRANSFER"),
    };
    let auth_req = AuthRequest::Spend(spend_auth);
    let args_vec: Vec<Val> = vec![
        &e,
        auth_req
            .try_into_val(&e)
            .unwrap_or_else(|_| panic!("intoval")),
    ];

    let root_invocation = xdr::SorobanAuthorizedInvocation {
        function: xdr::SorobanAuthorizedFunction::ContractFn(xdr::InvokeContractArgs {
            contract_address: utxo_mock.address.clone().try_into().unwrap(),
            function_name: "transfer".try_into().unwrap(),
            args: args_vec.try_into().unwrap(), //VecM::try_from(vec![&e, invocation_args.clone()]).unwrap(),
        }),
        sub_invocations: VecM::default(),
    };

    // let payload = HashIdPreimage::SorobanAuthorization(HashIdPreimageSorobanAuthorization {
    //     network_id: env.ledger().network_id().to_array().into(),
    //     nonce,
    //     signature_expiration_ledger,
    //     invocation: root_invocation.clone(),
    // });

    // let payload_xdr = payload
    //     .to_xdr(Limits {
    //         depth: u32::MAX,
    //         len: usize::MAX,
    //     })
    //     .unwrap();

    // let mut payload = Bytes::new(&env);

    // for byte in payload_xdr.iter() {
    //     payload.push_back(*byte);
    // }

    // let payload = env.crypto().sha256(&payload);

    // let address = Strkey::PublicKeyEd25519(ed25519::PublicKey(keypair.public.to_bytes()));
    // let address = Bytes::from_slice(&env, address.to_string().as_bytes());
    // let address = Address::from_string_bytes(&address);

    let address_auth_val = auth
        .address
        .clone()
        .try_into()
        .unwrap_or_else(|_| panic!("intoval"));

    let signature_val = signatures.try_into().unwrap_or_else(|_| panic!("intoval"));

    let result = utxo_mock
        .set_auths(&[xdr::SorobanAuthorizationEntry {
            credentials: xdr::SorobanCredentials::Address(SorobanAddressCredentials {
                address: address_auth_val,
                nonce,
                signature_expiration_ledger,
                signature: signature_val,
            }),
            root_invocation,
        }])
        .transfer(&invocation_args);

    assert!(result);
}
