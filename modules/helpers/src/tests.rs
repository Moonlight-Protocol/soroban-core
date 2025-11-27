extern crate std;
use crate::parser::{address_from_ed25519_pk_bytes, address_to_ed25519_pk_bytes};
use stellar_strkey::{ed25519::PublicKey as StrEd25519, Strkey};

use soroban_sdk::{Address, BytesN, Env};

#[test]
fn roundtrip_pk_to_address_and_back() {
    let e = Env::default();
    // deterministic sample key bytes
    let raw_pk: [u8; 32] = [
        224, 195, 160, 124, 85, 139, 22, 251, 230, 32, 222, 96, 135, 62, 255, 167, 222, 37, 35, 0,
        176, 226, 51, 233, 101, 9, 37, 80, 96, 181, 224, 110,
    ];
    let pk_bytes = BytesN::<32>::from_array(&e, &raw_pk);

    // Convert bytes -> Address
    let addr = address_from_ed25519_pk_bytes(&e, &pk_bytes);

    // Convert back to bytes
    let out_bytes = address_to_ed25519_pk_bytes(&e, &addr);

    assert_eq!(out_bytes.to_array(), raw_pk);
}

#[test]
fn address_string_roundtrip_works() {
    let e = Env::default();
    // create a strkey string from raw bytes and then parse via Address
    let raw_pk: [u8; 32] = [
        224, 195, 160, 124, 85, 139, 22, 251, 230, 32, 222, 96, 135, 62, 255, 167, 222, 37, 35, 0,
        176, 226, 51, 233, 101, 9, 37, 80, 96, 181, 224, 110,
    ];

    // Create Strkey string
    let str_pk = Strkey::PublicKeyEd25519(StrEd25519(raw_pk));
    let addr_str = str_pk.to_string();

    // Build Address from string and check conversion back yields same raw pk
    let addr: Address = Address::from_str(&e, &addr_str);

    let out_bytes = address_to_ed25519_pk_bytes(&e, &addr);

    assert_eq!(out_bytes.to_array(), raw_pk);
}

#[test]
fn start_from_address_string() {
    let e = Env::default();

    // Start with a known Stellar address string
    let addr_str = "GDQMHID4KWFRN67GEDPGBBZ676T54JJDACYOEM7JMUESKUDAWXQG5Z6Y";

    // Parse the address string into an Address
    let addr: Address = Address::from_str(&e, addr_str);

    // Convert to bytes
    let pk_bytes = address_to_ed25519_pk_bytes(&e, &addr);

    // Convert back to address
    let addr_roundtrip = address_from_ed25519_pk_bytes(&e, &pk_bytes);

    // Verify roundtrip matches
    assert_eq!(addr_roundtrip.to_string(), addr.to_string());
}
