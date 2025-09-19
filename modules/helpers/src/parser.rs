use soroban_sdk::{Address, BytesN, Env};

use stellar_strkey::{ed25519::PublicKey as StrEd25519, Strkey};

pub fn address_from_ed25519_pk_bytes(e: &Env, provider_pk32: &BytesN<32>) -> Address {
    let str_pk = Strkey::PublicKeyEd25519(StrEd25519(provider_pk32.to_array()));
    let provider_addr: Address = Address::from_str(&e, &str_pk.to_string());

    provider_addr
}

pub fn address_to_ed25519_pk_bytes(e: &Env, addr: &Address) -> BytesN<32> {
    let addr_string = addr.to_string();

    // Convert soroban_sdk::String to Bytes, then to fixed array
    let bytes_obj = addr_string.to_bytes();

    // Stellar addresses are 56 characters (G... format)
    let mut bytes_array = [0u8; 56];
    let len = bytes_obj.len() as usize;
    for i in 0..len.min(56) {
        bytes_array[i] = bytes_obj.get_unchecked(i as u32);
    }

    // Create a &str from the byte array
    let rust_str = core::str::from_utf8(&bytes_array[..len]).expect("Invalid UTF-8");
    let str_key = Strkey::from_string(rust_str).expect("Invalid address string");

    if let Strkey::PublicKeyEd25519(StrEd25519(provider_pk32)) = str_key {
        return BytesN::<32>::from_array(e, &provider_pk32);
    } else {
        panic!("not an ed25519 account address")
    }
}
