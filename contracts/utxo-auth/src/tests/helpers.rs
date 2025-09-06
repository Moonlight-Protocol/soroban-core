#![cfg(test)]
use core::panic;

use soroban_sdk::{
    contract, contracterror, contractimpl, symbol_short, testutils::Address as TestAddress, vec,
    Address, Env, TryIntoVal, Val, Vec,
};

use crate::contract::{
    AuthRequest, Bundle, UTXOAuthContract, UTXOAuthContractArgs, UTXOAuthContractClient,
    UtxoSpendAuth,
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

#[contractimpl]
impl TestContract {
    pub fn auth_address(e: Env) -> Address {
        e.storage().instance().get(&"utxo_auth").unwrap()
    }

    pub fn transfer(e: Env, bundles: Vec<Bundle>) -> bool {
        let utxo_auth: Address = e.storage().instance().get(&"utxo_auth").unwrap();
        let action = symbol_short!("TRANSFER");
        for bundle in bundles.iter() {
            for (_i, spend_utxo) in bundle.spend.iter().enumerate() {
                let spend_auth: UtxoSpendAuth = UtxoSpendAuth {
                    pk: spend_utxo.clone(),
                    bundle: bundle.clone(),
                    action: action.clone(),
                };
                let auth_req = AuthRequest::Spend(spend_auth);
                let args_vec: Vec<Val> = vec![
                    &e,
                    auth_req
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
