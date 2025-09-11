use soroban_sdk::{
    xdr::{AccountId, PublicKey as XdrPublicKey, ScAddress, ScVal, Uint256},
    Address, BytesN, Env,
};

use stellar_strkey::{ed25519::PublicKey as StrEd25519, Strkey};

pub fn address_from_ed25519_pk_bytes(e: &Env, provider_pk32: &BytesN<32>) -> Address {
    let str_pk = Strkey::PublicKeyEd25519(StrEd25519(provider_pk32.to_array()));
    let provider_addr: Address = Address::from_str(&e, &str_pk.to_string());

    provider_addr
}

pub fn address_to_ed25519_pk_bytes(e: &Env, addr: &Address) -> Result<BytesN<32>, &'static str> {
    let scv: ScVal = addr.into();
    let ScVal::Address(a) = scv else {
        return Err("not an Address ScVal");
    };

    if let ScAddress::Account(AccountId(XdrPublicKey::PublicKeyTypeEd25519(Uint256(pk)))) = a {
        Ok(BytesN::from_array(e, &pk))
    } else {
        Err("not an ed25519 account address")
    }
}
