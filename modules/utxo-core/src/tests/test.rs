use moonlight_helpers::testutils::keys::P256KeyPair;
use moonlight_primitives::Condition;
use soroban_sdk::{
    testutils::Address as _,
    vec,
    xdr::{self, VecM},
    Address, Env, Error, TryIntoVal, Val, Vec,
};

use crate::{
    core::{calculate_auth_requirements, Error as ContractError, UTXOOperation},
    testutils::contract::create_contract_with_mocked_auth,
};

#[test]
fn test_mint_and_burn() {
    let e = Env::default();
    let (client, _) = create_contract_with_mocked_auth(&e);

    // Non-existing UTXO should return -1
    let utxo = P256KeyPair::generate(&e);
    assert_eq!(client.utxo_balance(&utxo.public_key.clone()), -1_i128);

    // Minting should create the UTXO with the correct balance
    client.mint(&vec![&e, (utxo.public_key.clone(), 250_i128)]);
    assert_eq!(client.utxo_balance(&utxo.public_key.clone()), 250_i128);

    // Burning should set the UTXO balance to 0
    client.burn(&vec![&e, utxo.public_key.clone()]);
    assert_eq!(client.utxo_balance(&utxo.public_key.clone()), 0_i128);

    // Trying to mint an existing UTXO should return an error
    let expected_error_exists = client.try_mint(&vec![&e, (utxo.public_key.clone(), 500_i128)]);
    assert_eq!(
        expected_error_exists.err(),
        Some(Ok(Error::from_contract_error(
            ContractError::UTXOAlreadyExists as u32
        )))
    );
    assert_eq!(client.utxo_balance(&utxo.public_key.clone()), 0_i128);

    // Trying to burn an already spent UTXO should return an error
    let expected_error_spent = client.try_burn(&vec![&e, (utxo.public_key.clone())]);
    assert_eq!(
        expected_error_spent.err(),
        Some(Ok(Error::from_contract_error(
            ContractError::UTXOAlreadySpent as u32
        )))
    );
    assert_eq!(client.utxo_balance(&utxo.public_key.clone()), 0_i128);

    // Trying to burn a non-existing UTXO should return an error
    let new_utxo = P256KeyPair::generate(&e);
    let expected_error_not_exists = client.try_burn(&vec![&e, (new_utxo.public_key.clone())]);
    assert_eq!(
        expected_error_not_exists.err(),
        Some(Ok(Error::from_contract_error(
            ContractError::UTXODoesntExist as u32
        )))
    );

    // Trying to mint a UTXO with non-positive amount should return an error
    let expected_error_invalid_create_amount_negative =
        client.try_mint(&vec![&e, (new_utxo.public_key.clone(), -1)]);
    assert_eq!(
        expected_error_invalid_create_amount_negative.err(),
        Some(Ok(Error::from_contract_error(
            ContractError::InvalidCreateAmount as u32
        )))
    );

    // Zero amount is also invalid
    let expected_error_invalid_create_amount_zero =
        client.try_mint(&vec![&e, (new_utxo.public_key.clone(), 0)]);
    assert_eq!(
        expected_error_invalid_create_amount_zero.err(),
        Some(Ok(Error::from_contract_error(
            ContractError::InvalidCreateAmount as u32
        )))
    );
}

#[test]
fn test_transfer() {
    let e = Env::default();
    let (client, _) = create_contract_with_mocked_auth(&e);

    let utxo_a = P256KeyPair::generate(&e);
    let utxo_b = P256KeyPair::generate(&e);

    client.mint(&vec![&e, (utxo_a.public_key.clone(), 250_i128)]);
    assert_eq!(client.utxo_balance(&utxo_a.public_key.clone()), 250_i128);
    assert_eq!(client.utxo_balance(&utxo_b.public_key.clone()), -1_i128);

    let op = UTXOOperation {
        create: vec![&e, (utxo_b.public_key.clone(), 250_i128)],
        spend: vec![&e, (utxo_a.public_key.clone(), vec![&e])],
    };

    client.mock_all_auths().transact(&op);

    assert_eq!(client.utxo_balance(&utxo_a.public_key.clone()), 0_i128);
    assert_eq!(client.utxo_balance(&utxo_b.public_key.clone()), 250_i128);

    let utxo_c = P256KeyPair::generate(&e);
    let utxo_d = P256KeyPair::generate(&e);
    let utxo_e = P256KeyPair::generate(&e);
    let utxo_f = P256KeyPair::generate(&e);

    client.mint(&vec![&e, (utxo_c.public_key.clone(), 300_i128)]);
    assert_eq!(client.utxo_balance(&utxo_c.public_key.clone()), 300_i128);

    // Repeating UTXOs to create should faild
    let repeating_create_op = UTXOOperation {
        create: vec![
            &e,
            (utxo_d.public_key.clone(), 200_i128),
            (utxo_d.public_key.clone(), 20_i128),
        ],
        spend: vec![
            &e,
            (utxo_b.public_key.clone(), vec![&e]),
            (utxo_c.public_key.clone(), vec![&e]),
        ],
    };

    let expected_error_repeating_create =
        client.mock_all_auths().try_transact(&repeating_create_op);

    assert_eq!(
        expected_error_repeating_create.err(),
        Some(Ok(Error::from_contract_error(
            ContractError::RepeatedCreateUTXO as u32
        )))
    );

    // Repeating UTXOs to spend should faild
    let repeating_spend_op = UTXOOperation {
        create: vec![
            &e,
            (utxo_d.public_key.clone(), 200_i128),
            (utxo_e.public_key.clone(), 20_i128),
        ],
        spend: vec![
            &e,
            (utxo_b.public_key.clone(), vec![&e]),
            (utxo_b.public_key.clone(), vec![&e]),
        ],
    };

    let expected_error_repeating_spend = client.mock_all_auths().try_transact(&repeating_spend_op);

    assert_eq!(
        expected_error_repeating_spend.err(),
        Some(Ok(Error::from_contract_error(
            ContractError::RepeatedSpendUTXO as u32
        )))
    );

    // Spends 550 but tries to create 500, should fail
    let unbalanced_op = UTXOOperation {
        create: vec![
            &e,
            (utxo_d.public_key.clone(), 200_i128),
            (utxo_e.public_key.clone(), 200_i128),
            (utxo_f.public_key.clone(), 100_i128),
        ],
        spend: vec![
            &e,
            (utxo_b.public_key.clone(), vec![&e]),
            (utxo_c.public_key.clone(), vec![&e]),
        ],
    };

    let expected_error_unbalanced = client.mock_all_auths().try_transact(&unbalanced_op);

    assert_eq!(
        expected_error_unbalanced.err(),
        Some(Ok(Error::from_contract_error(
            ContractError::UnbalancedBundle as u32
        )))
    );

    let balanced_op = UTXOOperation {
        create: vec![
            &e,
            (utxo_d.public_key.clone(), 200_i128),
            (utxo_e.public_key.clone(), 200_i128),
            (utxo_f.public_key.clone(), 150_i128),
        ],
        spend: vec![
            &e,
            (utxo_b.public_key.clone(), vec![&e]),
            (utxo_c.public_key.clone(), vec![&e]),
        ],
    };

    client.mock_all_auths().transact(&balanced_op);

    assert_eq!(client.utxo_balance(&utxo_b.public_key.clone()), 0_i128);
    assert_eq!(client.utxo_balance(&utxo_c.public_key.clone()), 0_i128);
    assert_eq!(client.utxo_balance(&utxo_d.public_key.clone()), 200_i128);
    assert_eq!(client.utxo_balance(&utxo_e.public_key.clone()), 200_i128);
    assert_eq!(client.utxo_balance(&utxo_f.public_key.clone()), 150_i128);
}

#[test]
fn test_transfer_auth_mocked() {
    let e = Env::default();
    let (client, auth_client) = create_contract_with_mocked_auth(&e);

    let utxo_a = P256KeyPair::generate(&e);
    let utxo_b = P256KeyPair::generate(&e);
    let utxo_c = P256KeyPair::generate(&e);
    let utxo_d = P256KeyPair::generate(&e);
    let utxo_e = P256KeyPair::generate(&e);

    client.mint(&vec![&e, (utxo_a.public_key.clone(), 500_i128)]);
    client.mint(&vec![&e, (utxo_b.public_key.clone(), 800_i128)]);

    assert_eq!(client.utxo_balance(&utxo_a.public_key.clone()), 500_i128);
    assert_eq!(client.utxo_balance(&utxo_b.public_key.clone()), 800_i128);

    let op = UTXOOperation {
        create: vec![
            &e,
            (utxo_c.public_key.clone(), 400_i128),
            (utxo_d.public_key.clone(), 450_i128),
            (utxo_e.public_key.clone(), 450_i128),
        ],
        spend: vec![
            &e,
            (
                utxo_a.public_key.clone(),
                vec![
                    &e,
                    Condition::Create(utxo_c.public_key.clone(), 400),
                    Condition::Create(utxo_d.public_key.clone(), 450),
                ],
            ),
            (
                utxo_b.public_key.clone(),
                vec![
                    &e,
                    Condition::Create(utxo_d.public_key.clone(), 450),
                    Condition::Create(utxo_e.public_key.clone(), 450),
                ],
            ),
        ],
    };

    // Calling with no auth provided should fail
    let expected_auth_error = client.try_transact(&op);

    assert_eq!(
        expected_auth_error.err(),
        Some(Ok(Error::from_type_and_code(
            xdr::ScErrorType::Context,
            xdr::ScErrorCode::InvalidAction
        )))
    );

    let auth_contract_address_val: xdr::ScAddress = auth_client
        .address
        .clone()
        .try_into()
        .unwrap_or_else(|_| panic!("intoval"));

    let signature_expiration_ledger = e.ledger().sequence() + 1;

    let auth_req = calculate_auth_requirements(&e, &op.spend); //&vec![&e]);
    let args: Vec<Val> = vec![
        &e,
        auth_req
            .try_into_val(&e)
            .unwrap_or_else(|_| panic!("intoval")),
    ];

    let root_invocation = xdr::SorobanAuthorizedInvocation {
        function: xdr::SorobanAuthorizedFunction::ContractFn(xdr::InvokeContractArgs {
            contract_address: client.address.clone().try_into().unwrap(),
            function_name: "transact".try_into().unwrap(),
            args: args.try_into().unwrap(),
        }),
        sub_invocations: VecM::default(),
    };

    // Sending a correctly assembled auth entry with an invalid signature should fail
    let expected_unauthorized_error = client
        .set_auths(&[xdr::SorobanAuthorizationEntry {
            credentials: xdr::SorobanCredentials::Address(xdr::SorobanAddressCredentials {
                address: auth_contract_address_val.clone(),
                nonce: 0,
                signature_expiration_ledger,
                signature: false.try_into().unwrap_or_else(|_| panic!("intoval")), // Mocked as UNAUTHORIZED
            }),
            root_invocation: root_invocation.clone(),
        }])
        .try_transact(&op);

    assert_eq!(
        expected_unauthorized_error.err(),
        Some(Ok(Error::from_type_and_code(
            xdr::ScErrorType::Context,
            xdr::ScErrorCode::InvalidAction
        )))
    );

    // Sending a correctly assembled auth entry with a valid signature should work
    client
        .set_auths(&[xdr::SorobanAuthorizationEntry {
            credentials: xdr::SorobanCredentials::Address(xdr::SorobanAddressCredentials {
                address: auth_contract_address_val,
                nonce: 0,
                signature_expiration_ledger,
                signature: true.try_into().unwrap_or_else(|_| panic!("intoval")), // Mocked as AUTHORIZED
            }),
            root_invocation,
        }])
        .transact(&op);

    assert_eq!(client.utxo_balance(&utxo_a.public_key.clone()), 0_i128);
    assert_eq!(client.utxo_balance(&utxo_b.public_key.clone()), 0_i128);
    assert_eq!(client.utxo_balance(&utxo_c.public_key.clone()), 400_i128);
    assert_eq!(client.utxo_balance(&utxo_d.public_key.clone()), 450_i128);
    assert_eq!(client.utxo_balance(&utxo_e.public_key.clone()), 450_i128);
}

#[test]
fn test_transfer_with_external() {
    let e = Env::default();
    let (client, _) = create_contract_with_mocked_auth(&e);

    let utxo_a = P256KeyPair::generate(&e);
    let utxo_b = P256KeyPair::generate(&e);
    let utxo_c = P256KeyPair::generate(&e);

    client.mint(&vec![&e, (utxo_a.public_key.clone(), 250_i128)]);
    client.mint(&vec![&e, (utxo_b.public_key.clone(), 250_i128)]);
    assert_eq!(client.utxo_balance(&utxo_a.public_key.clone()), 250_i128);
    assert_eq!(client.utxo_balance(&utxo_b.public_key.clone()), 250_i128);
    assert_eq!(client.utxo_balance(&utxo_c.public_key.clone()), -1_i128);

    // Assemble a bundle which needs an additional 100 to be created
    let op_additional_in = UTXOOperation {
        create: vec![&e, (utxo_c.public_key.clone(), 600_i128)],
        spend: vec![
            &e,
            (utxo_a.public_key.clone(), vec![&e]),
            (utxo_b.public_key.clone(), vec![&e]),
        ],
    };

    // Calling without providing the additional input should fail
    let expected_unbalanced_error_in =
        client
            .mock_all_auths()
            .try_transact_with_external(&op_additional_in, &0_i128, &0_i128);

    assert_eq!(
        expected_unbalanced_error_in.err(),
        Some(Ok(Error::from_contract_error(
            ContractError::UnbalancedBundle as u32
        )))
    );

    // Calling with the additional input should succeed
    client
        .mock_all_auths()
        .transact_with_external(&op_additional_in, &100_i128, &0_i128);

    assert_eq!(client.utxo_balance(&utxo_a.public_key.clone()), 0_i128);
    assert_eq!(client.utxo_balance(&utxo_b.public_key.clone()), 0_i128);
    assert_eq!(client.utxo_balance(&utxo_c.public_key.clone()), 600_i128);

    let utxo_d = P256KeyPair::generate(&e);
    let utxo_e = P256KeyPair::generate(&e);
    let utxo_f = P256KeyPair::generate(&e);

    // Assemble a bundle which leaves out an additional 100 to be removed
    let op_additional_out = UTXOOperation {
        create: vec![
            &e,
            (utxo_d.public_key.clone(), 200_i128),
            (utxo_e.public_key.clone(), 200_i128),
            (utxo_f.public_key.clone(), 100_i128),
        ],
        spend: vec![&e, (utxo_c.public_key.clone(), vec![&e])],
    };

    // Calling without providing the additional output should fail
    let expected_unbalanced_error_out =
        client
            .mock_all_auths()
            .try_transact_with_external(&op_additional_out, &0_i128, &0_i128);

    assert_eq!(
        expected_unbalanced_error_out.err(),
        Some(Ok(Error::from_contract_error(
            ContractError::UnbalancedBundle as u32
        )))
    );

    // Calling with the additional output should succeed
    client
        .mock_all_auths()
        .transact_with_external(&op_additional_out, &0_i128, &100_i128);
    assert_eq!(client.utxo_balance(&utxo_c.public_key.clone()), 0_i128);
    assert_eq!(client.utxo_balance(&utxo_d.public_key.clone()), 200_i128);
    assert_eq!(client.utxo_balance(&utxo_e.public_key.clone()), 200_i128);
    assert_eq!(client.utxo_balance(&utxo_f.public_key.clone()), 100_i128);

    let utxo_g = P256KeyPair::generate(&e);

    //Assemble a bundle which needs an additional 100 to be created and leaves out an additional 50 to be removed
    let op_both = UTXOOperation {
        create: vec![&e, (utxo_g.public_key.clone(), 250_i128)],
        spend: vec![&e, (utxo_d.public_key.clone(), vec![&e])],
    };

    // Calling without providing the additional input and output should fail
    let expected_unbalanced_error_both = client
        .mock_all_auths()
        .try_transact_with_external(&op_both, &0_i128, &0_i128);

    assert_eq!(
        expected_unbalanced_error_both.err(),
        Some(Ok(Error::from_contract_error(
            ContractError::UnbalancedBundle as u32
        )))
    );

    // Calling with the additional input and output should succeed
    client
        .mock_all_auths()
        .transact_with_external(&op_both, &100_i128, &50_i128);
    assert_eq!(client.utxo_balance(&utxo_d.public_key.clone()), 0_i128);
    assert_eq!(client.utxo_balance(&utxo_g.public_key.clone()), 250_i128);
}

#[test]
fn test_transfer_with_additional_auth_conditions() {
    let e = Env::default();
    let (client, auth_client) = create_contract_with_mocked_auth(&e);

    let utxo_a = P256KeyPair::generate(&e);
    let utxo_b = P256KeyPair::generate(&e);
    let utxo_c = P256KeyPair::generate(&e);
    let utxo_d = P256KeyPair::generate(&e);
    let utxo_e = P256KeyPair::generate(&e);

    let utxo_integr_a = P256KeyPair::generate(&e);
    let utxo_integr_b = P256KeyPair::generate(&e);

    let account_a = Address::generate(&e);
    let account_b = Address::generate(&e);
    let account_c = Address::generate(&e);

    let integration_address = Address::generate(&e);

    client.mint(&vec![&e, (utxo_a.public_key.clone(), 500_i128)]);
    client.mint(&vec![&e, (utxo_b.public_key.clone(), 800_i128)]);

    assert_eq!(client.utxo_balance(&utxo_a.public_key.clone()), 500_i128);
    assert_eq!(client.utxo_balance(&utxo_b.public_key.clone()), 800_i128);

    // Assemble with additional auth conditions for external deposits, withdrawals and integration
    // These should affect the auth requirements but not the balance calculations
    let op = UTXOOperation {
        create: vec![
            &e,
            (utxo_c.public_key.clone(), 400_i128),
            (utxo_d.public_key.clone(), 450_i128),
            (utxo_e.public_key.clone(), 450_i128),
        ],
        spend: vec![
            &e,
            (
                utxo_a.public_key.clone(),
                vec![
                    &e,
                    Condition::Create(utxo_c.public_key.clone(), 400),
                    Condition::Create(utxo_d.public_key.clone(), 450),
                    Condition::ExtDeposit(account_a, 200_i128),
                    Condition::ExtDeposit(account_b, 100_i128),
                ],
            ),
            (
                utxo_b.public_key.clone(),
                vec![
                    &e,
                    Condition::Create(utxo_d.public_key.clone(), 450),
                    Condition::Create(utxo_e.public_key.clone(), 450),
                    Condition::ExtWithdraw(account_c, 150_i128),
                    Condition::ExtIntegration(
                        integration_address,
                        vec![
                            &e,
                            utxo_integr_a.public_key.clone(),
                            utxo_integr_b.public_key.clone(),
                        ],
                        150_i128,
                    ),
                ],
            ),
        ],
    };

    let auth_contract_address_val: xdr::ScAddress = auth_client
        .address
        .clone()
        .try_into()
        .unwrap_or_else(|_| panic!("intoval"));

    let signature_expiration_ledger = e.ledger().sequence() + 1;

    let auth_req = calculate_auth_requirements(&e, &op.spend); // &vec![&e]);
    let args: Vec<Val> = vec![
        &e,
        auth_req
            .try_into_val(&e)
            .unwrap_or_else(|_| panic!("intoval")),
    ];

    let root_invocation = xdr::SorobanAuthorizedInvocation {
        function: xdr::SorobanAuthorizedFunction::ContractFn(xdr::InvokeContractArgs {
            contract_address: client.address.clone().try_into().unwrap(),
            function_name: "transact_with_external".try_into().unwrap(),
            args: args.try_into().unwrap(),
        }),
        sub_invocations: VecM::default(),
    };

    client
        .set_auths(&[xdr::SorobanAuthorizationEntry {
            credentials: xdr::SorobanCredentials::Address(xdr::SorobanAddressCredentials {
                address: auth_contract_address_val,
                nonce: 0,
                signature_expiration_ledger,
                signature: true.try_into().unwrap_or_else(|_| panic!("intoval")), // Mocked as AUTHORIZED
            }),
            root_invocation,
        }])
        .transact_with_external(&op, &300_i128, &300_i128);

    assert_eq!(client.utxo_balance(&utxo_a.public_key.clone()), 0_i128);
    assert_eq!(client.utxo_balance(&utxo_b.public_key.clone()), 0_i128);
    assert_eq!(client.utxo_balance(&utxo_c.public_key.clone()), 400_i128);
    assert_eq!(client.utxo_balance(&utxo_d.public_key.clone()), 450_i128);
    assert_eq!(client.utxo_balance(&utxo_e.public_key.clone()), 450_i128);
}
