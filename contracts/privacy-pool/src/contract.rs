use crate::storage::{
    is_contract_initialized, read_admin, read_asset, read_provider_balance, read_supply,
    write_admin_unchecked, write_asset_unchecked,
};
use crate::treasury::{
    decrease_provider_balance, decrease_supply, increase_provider_balance, increase_supply,
};

use provider_management::core::is_provider;
use provider_management::core::{
    deregister_provider as _deregister_provider, is_provider as _is_provider,
    register_provider as _register_provider, ProviderManagementTrait,
};
use soroban_sdk::crypto::Hash;
use soroban_sdk::token::TokenClient;
use soroban_sdk::{contract, contractimpl, vec, Address, BytesN, Env, Vec};
use utxo::core::{
    burn as _burn, burn_payload, delegated_transfer as _delegated_transfer, mint as _mint,
    transfer as _transfer, transfer_burn_leftover, utxo_balance, Bundle,
};

pub trait PrivacyPoolCoreTrait {
    fn __constructor(e: Env, admin: Address, asset: Address);

    fn admin(e: Env) -> Address;

    fn supply(e: Env) -> i128;

    fn deposit(e: Env, from: Address, amount: i128, utxo: BytesN<65>);

    fn withdraw(e: Env, to: Address, amount: i128, utxo: BytesN<65>, signature: BytesN<64>);

    fn balance(e: Env, utxo: BytesN<65>) -> i128;

    fn balances(e: Env, utxos: Vec<BytesN<65>>) -> Vec<i128>;

    fn transfer(e: Env, bundles: Vec<Bundle>);

    fn delegated_transfer_utxo(
        e: Env,
        bundles: Vec<Bundle>,
        provider: Address,
        delegate_utxo: BytesN<65>,
    );

    fn delegated_transfer_bal(e: Env, bundles: Vec<Bundle>, provider: Address);

    fn provider_balance(e: Env, provider: Address) -> i128;

    fn provider_withdraw(e: Env, provider: Address, amount: i128);

    fn build_withdraw_payload(e: Env, utxo: BytesN<65>, amount: i128) -> BytesN<32>;
}

#[contract]
pub struct PrivacyPoolContract;

#[contractimpl]
impl PrivacyPoolCoreTrait for PrivacyPoolContract {
    fn __constructor(e: Env, admin: Address, asset: Address) {
        assert!(
            !is_contract_initialized(&e),
            "Contract already initialized!"
        );

        admin.require_auth();
        write_admin_unchecked(&e, admin);
        write_asset_unchecked(&e, asset);
    }

    fn admin(e: Env) -> Address {
        read_admin(&e)
    }

    fn supply(e: Env) -> i128 {
        read_supply(&e)
    }

    fn deposit(e: Env, from: Address, amount: i128, utxo: BytesN<65>) {
        assert!(is_contract_initialized(&e), "Contract not initialized!");

        from.require_auth();
        let asset = read_asset(&e);

        let asset_client = TokenClient::new(&e, &asset);

        asset_client.transfer(&from, &e.current_contract_address(), &amount);

        _mint(&e, amount, utxo);
        increase_supply(&e, amount);
    }

    fn withdraw(e: Env, to: Address, amount: i128, utxo: BytesN<65>, signature: BytesN<64>) {
        assert!(is_contract_initialized(&e), "Contract not initialized!");

        let asset = read_asset(&e);

        let balance = utxo_balance(e.clone(), utxo.clone());

        assert!(balance == amount, "Incorrect UTXO balance!");

        _burn(&e, utxo, signature);
        decrease_supply(&e, amount);

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

    fn delegated_transfer_utxo(
        e: Env,
        bundles: Vec<Bundle>,
        provider: Address,
        delegate_utxo: BytesN<65>,
    ) {
        assert!(is_contract_initialized(&e), "Contract not initialized!");

        assert!(
            is_provider(&e, provider.clone()),
            "Provider not registered!"
        );

        provider.require_auth();

        _delegated_transfer(&e, bundles, delegate_utxo);
    }

    fn delegated_transfer_bal(e: Env, bundles: Vec<Bundle>, provider: Address) {
        assert!(is_contract_initialized(&e), "Contract not initialized!");

        assert!(
            is_provider(&e, provider.clone()),
            "Provider not registered!"
        );

        provider.require_auth();

        let change = transfer_burn_leftover(&e, bundles, "DELEGATED_TRANSFER");

        increase_provider_balance(&e, provider, change);
    }

    fn provider_balance(e: Env, provider: Address) -> i128 {
        read_provider_balance(&e, provider)
    }

    fn provider_withdraw(e: Env, provider: Address, amount: i128) {
        is_provider(&e, provider.clone());
        provider.require_auth();

        let balance = read_provider_balance(&e, provider.clone());
        assert!(balance >= amount, "Insufficient balance!");

        decrease_supply(&e, amount);
        decrease_provider_balance(&e, provider.clone(), amount);

        let asset = read_asset(&e);
        let asset_client = TokenClient::new(&e, &asset);

        asset_client.transfer(&e.current_contract_address(), &provider, &amount);
    }

    fn build_withdraw_payload(e: Env, utxo: BytesN<65>, amount: i128) -> BytesN<32> {
        assert!(is_contract_initialized(&e), "Contract not initialized!");
        let hash = withdraw_payload(&e, utxo, amount);
        BytesN::from_array(&e, &hash.to_array())
    }
}

#[contractimpl]
impl ProviderManagementTrait for PrivacyPoolContract {
    fn register_provider(e: Env, provider: Address) {
        require_admin(&e);
        _register_provider(&e, provider);
    }

    fn deregister_provider(e: Env, provider: Address) {
        require_admin(&e);
        _deregister_provider(&e, provider);
    }

    fn is_provider(e: Env, provider: Address) -> bool {
        _is_provider(&e, provider)
    }
}

pub fn withdraw_payload(e: &Env, utxo: BytesN<65>, amount: i128) -> Hash<32> {
    burn_payload(&e, &utxo, amount)
}

pub fn require_admin(e: &Env) {
    read_admin(e).require_auth();
}
