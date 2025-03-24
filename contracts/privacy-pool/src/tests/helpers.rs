#![cfg(test)]
use soroban_sdk::{
    testutils::Address,
    token::{StellarAssetClient, TokenClient},
    Env,
};

use crate::contract::{PrivacyPoolContract, PrivacyPoolContractArgs, PrivacyPoolContractClient};

pub fn create_contracts(e: &Env) -> (PrivacyPoolContractClient, StellarAssetClient, TokenClient) {
    let admin = <soroban_sdk::Address as Address>::generate(&e);

    let asset_contract = e.register_stellar_asset_contract_v2(admin.clone());

    let contract_address = e.register(
        PrivacyPoolContract,
        PrivacyPoolContractArgs::__constructor(&asset_contract.address()),
    );

    let contract_client = PrivacyPoolContractClient::new(&e, &contract_address);

    let asset_client = StellarAssetClient::new(&e, &asset_contract.address());
    let token_client = TokenClient::new(&e, &asset_contract.address());

    (contract_client, asset_client, token_client)
}
