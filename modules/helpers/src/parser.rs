use moonlight_errors::Error as MoonlightError;
use soroban_sdk::{address_payload::AddressPayload, panic_with_error, Address, BytesN, Env};

pub fn address_from_ed25519_pk_bytes(e: &Env, provider_pk32: &BytesN<32>) -> Address {
    Address::from_payload(
        e,
        AddressPayload::AccountIdPublicKeyEd25519(provider_pk32.clone()),
    )
}

pub fn address_to_ed25519_pk_bytes(e: &Env, addr: &Address) -> BytesN<32> {
    match addr.to_payload() {
        Some(AddressPayload::AccountIdPublicKeyEd25519(provider_pk32)) => provider_pk32,
        Some(AddressPayload::ContractIdHash(_)) => {
            panic_with_error!(e, MoonlightError::NotEd25519AccountAddress)
        }
        None => panic_with_error!(e, MoonlightError::UnsupportedAddressPayload),
    }
}
