#![cfg(test)]

use soroban_sdk::{contract, contractimpl, Address, Env};

use crate::core::{
    deregister_provider, is_provider, register_provider, require_provider, ProviderManagementTrait,
};

#[contract]
pub struct TestContract;

#[contractimpl]
impl TestContract {
    pub fn __constructor(_e: Env) {}

    pub fn only_provider(e: Env, provider: Address) -> bool {
        require_provider(&e, provider);
        true
    }
}

#[contractimpl]
impl ProviderManagementTrait for TestContract {
    fn register_provider(e: Env, provider: Address) {
        register_provider(&e, provider);
    }

    fn deregister_provider(e: Env, provider: Address) {
        deregister_provider(&e, provider);
    }

    fn is_provider(e: Env, provider: Address) -> bool {
        is_provider(&e, provider)
    }
}

pub fn create_contract(e: &Env) -> TestContractClient {
    let contract_id = e.register(TestContract, TestContractArgs::__constructor());
    let contract = TestContractClient::new(e, &contract_id);
    // Initialize contract if needed
    contract
}
