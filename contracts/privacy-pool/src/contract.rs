use crate::storage::{is_contract_initialized, read_asset, write_asset};
use soroban_sdk::crypto::Hash;
use soroban_sdk::token::TokenClient;
use soroban_sdk::{contract, contractimpl, vec, Address, BytesN, Env, Vec};
use utxo::core::{
    burn as _burn, burn_payload, delegated_transfer as _delegated_transfer, mint as _mint,
    transfer as _transfer, utxo_balance, Bundle,
};

#[contract]
pub struct PrivacyPoolContract;

pub trait PrivacyPoolTrait {
    fn __constructor(e: Env, asset: Address);

    fn deposit(e: Env, from: Address, amount: i128, utxo: BytesN<65>);

    fn withdraw(e: Env, to: Address, amount: i128, utxo: BytesN<65>, signature: BytesN<64>);

    fn balance(e: Env, utxo: BytesN<65>) -> i128;

    fn balances(e: Env, utxos: Vec<BytesN<65>>) -> Vec<i128>;

    fn transfer(e: Env, bundles: Vec<Bundle>);

    fn delegated_transfer(e: Env, bundles: Vec<Bundle>, delegate_utxo: BytesN<65>);

    fn build_withdraw_payload(e: Env, utxo: BytesN<65>, amount: i128) -> BytesN<32>;
}

#[contractimpl]
impl PrivacyPoolTrait for PrivacyPoolContract {
    fn __constructor(e: Env, asset: Address) {
        assert!(
            !is_contract_initialized(&e),
            "Contract already initialized!"
        );

        write_asset(e, asset);
    }

    fn deposit(e: Env, from: Address, amount: i128, utxo: BytesN<65>) {
        assert!(is_contract_initialized(&e), "Contract not initialized!");

        from.require_auth();
        let asset = read_asset(&e);

        let asset_client = TokenClient::new(&e, &asset);

        asset_client.transfer(&from, &e.current_contract_address(), &amount);

        _mint(&e, amount, utxo);
    }

    fn withdraw(e: Env, to: Address, amount: i128, utxo: BytesN<65>, signature: BytesN<64>) {
        assert!(is_contract_initialized(&e), "Contract not initialized!");

        let asset = read_asset(&e);

        let balance = utxo_balance(e.clone(), utxo.clone());

        assert!(balance == amount, "Incorrect UTXO balance!");

        _burn(&e, utxo, signature);

        let asset_client = TokenClient::new(&e, &asset);

        asset_client.transfer(&e.current_contract_address(), &to, &amount);
    }

    fn balance(e: Env, utxo: BytesN<65>) -> i128 {
        assert!(is_contract_initialized(&e), "Contract not initialized!");

        utxo_balance(e, utxo)
    }

    fn balances(e: Env, utxos: Vec<BytesN<65>>) -> Vec<i128> {
        let mut balances: Vec<i128> = vec![&e];

        for u in utxos {
            balances.push_back(utxo_balance(e.clone(), u));
        }

        balances
    }

    fn transfer(e: Env, bundles: Vec<Bundle>) {
        assert!(is_contract_initialized(&e), "Contract not initialized!");

        _transfer(&e, bundles);
    }

    fn delegated_transfer(e: Env, bundles: Vec<Bundle>, delegate_utxo: BytesN<65>) {
        assert!(is_contract_initialized(&e), "Contract not initialized!");

        _delegated_transfer(&e, bundles, delegate_utxo);
    }

    fn build_withdraw_payload(e: Env, utxo: BytesN<65>, amount: i128) -> BytesN<32> {
        assert!(is_contract_initialized(&e), "Contract not initialized!");
        let hash = withdraw_payload(&e, utxo, amount);
        BytesN::from_array(&e, &hash.to_array())
    }
}

pub fn withdraw_payload(e: &Env, utxo: BytesN<65>, amount: i128) -> Hash<32> {
    burn_payload(&e, &utxo, amount)
}
