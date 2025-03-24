#![cfg(test)]
use soroban_sdk::{
    testutils::Address as TestAddress,
    token::{StellarAssetClient, TokenClient},
    Address, Env,
};

use crate::contract::{PrivacyPoolContract, PrivacyPoolContractArgs, PrivacyPoolContractClient};

pub fn create_contracts(
    e: &Env,
) -> (
    Address,
    PrivacyPoolContractClient,
    StellarAssetClient,
    TokenClient,
) {
    let admin = <soroban_sdk::Address as TestAddress>::generate(&e);

    let asset_contract = e.register_stellar_asset_contract_v2(admin.clone());

    let contract_address = e.register(
        PrivacyPoolContract,
        PrivacyPoolContractArgs::__constructor(&admin, &asset_contract.address()),
    );

    let contract_client = PrivacyPoolContractClient::new(&e, &contract_address);

    let asset_client = StellarAssetClient::new(&e, &asset_contract.address());
    let token_client = TokenClient::new(&e, &asset_contract.address());

    (admin, contract_client, asset_client, token_client)
}
