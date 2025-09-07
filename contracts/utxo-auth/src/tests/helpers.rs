#![cfg(test)]
use core::panic;

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short,
    testutils::Address as TestAddress, vec, Address, BytesN, Env, TryIntoVal, Val, Vec,
};

use crate::{
    contract::{UTXOAuthContract, UTXOAuthContractArgs, UTXOAuthContractClient},
    payload::{AuthPayload, SpendingCondition},
    signature::SignerKey,
};

#[contract]
pub struct TestContract;

#[contractimpl]
impl TestContract {
    pub fn __constructor(e: Env, utxo_auth: Address) {
        e.storage().instance().set(&"utxo_auth", &utxo_auth);
    }
}

#[contracterror]
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Error {
    Intoval = 1,
}

#[derive(Clone)]
#[contracttype]
pub struct SpendBundle {
    pub spend: Vec<BytesN<65>>,
    pub create: Vec<(BytesN<65>, i128)>,
}

#[contractimpl]
impl TestContract {
    pub fn auth_address(e: Env) -> Address {
        e.storage().instance().get(&"utxo_auth").unwrap()
    }

    pub fn transfer(e: Env, bundles: Vec<SpendBundle>) -> bool {
        let utxo_auth: Address = e.storage().instance().get(&"utxo_auth").unwrap();
        for bundle in bundles.iter() {
            let mut conditions: Vec<SpendingCondition> = vec![&e];
            for (_i, (create_utxo, amount)) in bundle.create.iter().enumerate() {
                conditions.push_back(SpendingCondition::Create(create_utxo.clone(), amount));
            }

            let spend_auth: AuthPayload = AuthPayload {
                contract: e.current_contract_address(),
                conditions: conditions.clone(),
            };

            for (_i, spend_utxo) in bundle.spend.iter().enumerate() {
                let args_vec: Vec<Val> = vec![
                    &e,
                    SignerKey::P256(spend_utxo.clone())
                        .try_into_val(&e)
                        .unwrap(),
                    spend_auth
                        .try_into_val(&e)
                        .unwrap_or_else(|_| panic!("intoval")),
                ];
                utxo_auth.require_auth_for_args(args_vec);
            }
        }
        true
    }
}

pub fn create_contracts(e: &Env) -> (Address, UTXOAuthContractClient, TestContractClient) {
    let admin = <soroban_sdk::Address as TestAddress>::generate(&e);

    let auth_contract_address = e.register(
        UTXOAuthContract,
        UTXOAuthContractArgs::__constructor(&admin),
    );

    let auth_contract_client = UTXOAuthContractClient::new(&e, &auth_contract_address);

    let mock_utxo_contract_address = e.register(
        TestContract,
        TestContractArgs::__constructor(&auth_contract_address),
    );

    let mock_utxo_contract_client = TestContractClient::new(&e, &mock_utxo_contract_address);

    (admin, auth_contract_client, mock_utxo_contract_client)
}
