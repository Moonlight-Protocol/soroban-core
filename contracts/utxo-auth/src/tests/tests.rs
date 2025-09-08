#![cfg(test)]
use signature::{Signature, Signatures, SignerKey};
use soroban_sdk::{
    symbol_short,
    testutils::{AuthorizedFunction, AuthorizedInvocation},
    vec,
    xdr::{self, SorobanAddressCredentials, ToXdr, VecM},
    Address, BytesN, Env, IntoVal, Map, TryIntoVal, Val, Vec,
};
extern crate std;

use utxo::tests::helpers::{generate_utxo_keypair, sign_hash};

use crate::{
    contract::UTXOAuthContractClient,
    payload::{hash_payload, AuthPayload, SpendingCondition},
    tests::helpers::{create_contracts, SpendBundle, TestContractClient},
};

#[test]
fn test_minimal_auth_bundle_success() {
    let e = Env::default();

    let (_admin, auth, utxo_mock): (Address, UTXOAuthContractClient, TestContractClient) =
        create_contracts(&e);

    assert_eq!(utxo_mock.auth_address(), auth.address);

    let utxo_keypair_a = generate_utxo_keypair(&e);
    let utxo_keypair_b = generate_utxo_keypair(&e);

    let mut bundle_a = SpendBundle {
        spend: vec![&e, utxo_keypair_a.public_key.clone()],
        create: vec![&e, (utxo_keypair_b.public_key.clone(), 250)],
    };

    let auth_payload: AuthPayload = AuthPayload {
        contract: utxo_mock.address.clone(),
        conditions: vec![
            &e,
            SpendingCondition::Create(utxo_keypair_b.public_key.clone(), 250),
        ],
    };

    let hash_a = hash_payload(&e, &auth_payload);

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

    // let spend_auth: UtxoSpendAuth = UtxoSpendAuth {
    //     pk: utxo_keypair_a.public_key.clone(),
    //     bundle: bundle_a.clone(),
    //     action: symbol_short!("TRANSFER"),
    // };
    // let auth_req = AuthRequest::Spend(spend_auth);
    let args_vec: Vec<Val> = vec![
        &e,
        SignerKey::P256(utxo_keypair_a.public_key.clone()).into_val(&e),
        auth_payload
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

#[test]
fn test_multiple_auth_bundle_success() {
    let e = Env::default();

    let (_admin, auth, utxo_mock): (Address, UTXOAuthContractClient, TestContractClient) =
        create_contracts(&e);

    assert_eq!(utxo_mock.auth_address(), auth.address);

    let utxo_keypair_a = generate_utxo_keypair(&e);
    let utxo_keypair_b = generate_utxo_keypair(&e);
    let utxo_keypair_c = generate_utxo_keypair(&e);
    let utxo_keypair_d = generate_utxo_keypair(&e);
    let utxo_keypair_e = generate_utxo_keypair(&e);
    let mut bundle_a = SpendBundle {
        spend: vec![
            &e,
            utxo_keypair_a.public_key.clone(),
            utxo_keypair_e.public_key.clone(),
        ],
        create: vec![
            &e,
            (utxo_keypair_b.public_key.clone(), 250),
            (utxo_keypair_c.public_key.clone(), 10),
        ],
    };

    let mut bundle_b = SpendBundle {
        spend: vec![
            &e,
            utxo_keypair_b.public_key.clone(),
            // utxo_keypair_c.public_key.clone(),
            // utxo_keypair_d.public_key.clone(),
        ],
        create: vec![
            &e,
            (utxo_keypair_c.public_key.clone(), 300),
            (utxo_keypair_a.public_key.clone(), 10),
        ],
    };

    let auth_payload_a: AuthPayload = AuthPayload {
        contract: utxo_mock.address.clone(),
        conditions: vec![
            &e,
            SpendingCondition::Create(utxo_keypair_b.public_key.clone(), 250),
            SpendingCondition::Create(utxo_keypair_c.public_key.clone(), 10),
        ],
    };

    let mut auth_payload_b = AuthPayload {
        contract: utxo_mock.address.clone(),
        conditions: vec![
            &e,
            SpendingCondition::Create(utxo_keypair_c.public_key.clone(), 300),
            SpendingCondition::Create(utxo_keypair_a.public_key.clone(), 10),
        ],
    };

    let hash_a = hash_payload(&e, &auth_payload_a);
    let hash_b = hash_payload(&e, &auth_payload_b);

    let signature_a: [u8; 64] = sign_hash(&utxo_keypair_a.secret_key, &hash_a);
    let signature_e: [u8; 64] = sign_hash(&utxo_keypair_e.secret_key, &hash_a);
    let signature_b: [u8; 64] = sign_hash(&utxo_keypair_b.secret_key, &hash_b);
    let signature_c: [u8; 64] = sign_hash(&utxo_keypair_c.secret_key, &hash_b);
    let signature_d: [u8; 64] = sign_hash(&utxo_keypair_d.secret_key, &hash_b);

    let signature_bytes_a = BytesN::<64>::from_array(&e, &signature_a);
    let signature_bytes_e = BytesN::<64>::from_array(&e, &signature_e);
    let signature_bytes_b = BytesN::<64>::from_array(&e, &signature_b);
    let signature_bytes_c = BytesN::<64>::from_array(&e, &signature_c);
    let signature_bytes_d = BytesN::<64>::from_array(&e, &signature_d);

    let invocation_args = vec![&e, bundle_a.clone()]; //vec![&e, bundle_a.clone(), bundle_b.clone()];

    let nonce = 0;
    let signature_expiration_ledger = e.ledger().sequence() + 100;

    let mut sign_map_a = Map::new(&e);
    sign_map_a.set(
        SignerKey::P256(utxo_keypair_a.public_key.clone()),
        Signature::P256(signature_bytes_a.clone()),
    );
    let mut sign_map_e = Map::new(&e);
    sign_map_e.set(
        SignerKey::P256(utxo_keypair_e.public_key.clone()),
        Signature::P256(signature_bytes_e.clone()),
    );
    // sign_map.set(
    //     SignerKey::P256(utxo_keypair_b.public_key.clone()),
    //     Signature::P256(signature_bytes_b.clone()),
    // );
    // sign_map.set(
    //     SignerKey::P256(utxo_keypair_c.public_key.clone()),
    //     Signature::P256(signature_bytes_c.clone()),
    // );
    // sign_map.set(
    //     SignerKey::P256(utxo_keypair_d.public_key.clone()),
    //     Signature::P256(signature_bytes_d.clone()),
    // );

    let signatures_a: Signatures = Signatures(sign_map_a);
    // let signatures_e: Signatures = Signatures(sign_map_e);
    // {
    //     pk: utxo_keypair_a.public_key.clone(),
    //     sig: signature_bytes_a,
    // };

    // let spend_auth: UtxoSpendAuth = UtxoSpendAuth {
    //     pk: utxo_keypair_a.public_key.clone(),
    //     bundle: bundle_a.clone(),
    //     action: symbol_short!("TRANSFER"),
    // };
    // let auth_req = AuthRequest::Spend(spend_auth);
    let args_vec_a_a: Vec<Val> = vec![
        &e,
        SignerKey::P256(utxo_keypair_a.public_key.clone()).into_val(&e),
        auth_payload_a
            .try_into_val(&e)
            .unwrap_or_else(|_| panic!("intoval")),
    ];
    let args_vec_a_e: Vec<Val> = vec![
        &e,
        SignerKey::P256(utxo_keypair_e.public_key.clone()).into_val(&e),
        auth_payload_a
            .try_into_val(&e)
            .unwrap_or_else(|_| panic!("intoval")),
    ];
    // let args_vec_b: Vec<Val> = vec![
    //     &e,
    //     SignerKey::P256(utxo_keypair_b.public_key.clone()).into_val(&e),
    //     auth_payload_b
    //         .try_into_val(&e)
    //         .unwrap_or_else(|_| panic!("intoval")),
    // ];
    // let args_vec_c: Vec<Val> = vec![
    //     &e,
    //     SignerKey::P256(utxo_keypair_c.public_key.clone()).into_val(&e),
    //     auth_payload_b
    //         .try_into_val(&e)
    //         .unwrap_or_else(|_| panic!("intoval")),
    // ];
    // let args_vec_d: Vec<Val> = vec![
    //     &e,
    //     SignerKey::P256(utxo_keypair_d.public_key.clone()).into_val(&e),
    //     auth_payload_b
    //         .try_into_val(&e)
    //         .unwrap_or_else(|_| panic!("intoval")),
    // ];

    // let root_invocation_e = xdr::SorobanAuthorizedInvocation {
    //     function: xdr::SorobanAuthorizedFunction::ContractFn(xdr::InvokeContractArgs {
    //         contract_address: utxo_mock.address.clone().try_into().unwrap(),
    //         function_name: "transfer".try_into().unwrap(),
    //         args: args_vec_a_e.try_into().unwrap(),
    //     }),
    //     sub_invocations: VecM::default(),
    // };

    // let root_invocation_b = xdr::SorobanAuthorizedInvocation {
    //     function: xdr::SorobanAuthorizedFunction::ContractFn(xdr::InvokeContractArgs {
    //         contract_address: utxo_mock.address.clone().try_into().unwrap(),
    //         function_name: "transfer".try_into().unwrap(),
    //         args: args_vec_b.try_into().unwrap(), //VecM::try_from(vec![&e, invocation_args.clone()]).unwrap(),
    //     }),
    //     sub_invocations: VecM::default(),
    // };

    // let root_invocation_c = xdr::SorobanAuthorizedInvocation {
    //     function: xdr::SorobanAuthorizedFunction::ContractFn(xdr::InvokeContractArgs {
    //         contract_address: utxo_mock.address.clone().try_into().unwrap(),
    //         function_name: "transfer".try_into().unwrap(),
    //         args: args_vec_c.try_into().unwrap(), //VecM::try_from(vec![&e, invocation_args.clone()]).unwrap(),
    //     }),
    //     sub_invocations: VecM::default(),
    // };

    // let root_invocation_d = xdr::SorobanAuthorizedInvocation {
    //     function: xdr::SorobanAuthorizedFunction::ContractFn(xdr::InvokeContractArgs {
    //         contract_address: utxo_mock.address.clone().try_into().unwrap(),
    //         function_name: "transfer".try_into().unwrap(),
    //         args: args_vec_d.try_into().unwrap(), //VecM::try_from(vec![&e, invocation_args.clone()]).unwrap(),
    //     }),
    //     sub_invocations: VecM::default(),
    // };

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

    // let address_auth_val =

    // let signature_val = signatures.try_into().unwrap_or_else(|_| panic!("intoval"));

    e.mock_all_auths();

    utxo_mock.transfer(&invocation_args);

    // assert_eq!(
    //     e.auths(),
    //     std::vec![(
    //         auth.address.clone(),
    //         AuthorizedInvocation {
    //             function: AuthorizedFunction::Contract((
    //                 utxo_mock.address.clone(),
    //                 symbol_short!("transfer"),
    //                 args_vec_a_a.into_val(&e)
    //             )),
    //             sub_invocations: std::vec![]
    //         }
    //     )]
    // );

    // let result = utxo_mock
    //     .set_auths(&[xdr::SorobanAuthorizationEntry {
    //         credentials: xdr::SorobanCredentials::Address(SorobanAddressCredentials {
    //             address: auth
    //                 .address
    //                 .clone()
    //                 .try_into()
    //                 .unwrap_or_else(|_| panic!("intoval")),
    //             nonce,
    //             signature_expiration_ledger,
    //             signature: signatures_a
    //                 .clone()
    //                 .try_into()
    //                 .unwrap_or_else(|_| panic!("intoval")),
    //         }),
    //         root_invocation: xdr::SorobanAuthorizedInvocation {
    //             function: xdr::SorobanAuthorizedFunction::ContractFn(xdr::InvokeContractArgs {
    //                 contract_address: utxo_mock.address.clone().try_into().unwrap(),
    //                 function_name: "transfer".try_into().unwrap(),
    //                 args: args_vec_a_a.clone().try_into().unwrap(),
    //             }),
    //             sub_invocations: VecM::default(),
    //         },
    //     }])
    //     .set_auths(&[xdr::SorobanAuthorizationEntry {
    //         credentials: xdr::SorobanCredentials::Address(SorobanAddressCredentials {
    //             address: auth
    //                 .address
    //                 .clone()
    //                 .try_into()
    //                 .unwrap_or_else(|_| panic!("intoval")),
    //             nonce,
    //             signature_expiration_ledger,
    //             signature: signatures_a
    //                 .clone()
    //                 .try_into()
    //                 .unwrap_or_else(|_| panic!("intoval")),
    //         }),
    //         root_invocation: xdr::SorobanAuthorizedInvocation {
    //             function: xdr::SorobanAuthorizedFunction::ContractFn(xdr::InvokeContractArgs {
    //                 contract_address: utxo_mock.address.clone().try_into().unwrap(),
    //                 function_name: "transfer".try_into().unwrap(),
    //                 args: args_vec_a_e.clone().try_into().unwrap(),
    //             }),
    //             sub_invocations: VecM::default(),
    //         },
    //     }])

    //     .transfer(&invocation_args);

    // assert!(result);
}
