extern crate std;
use crate::parser::{address_from_ed25519_pk_bytes, address_to_ed25519_pk_bytes};

use soroban_sdk::{address_payload::AddressPayload, Address, BytesN, Env};

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
fn start_from_address_string() {
    let e = Env::default();
    let raw_pk: [u8; 32] = [
        224, 195, 160, 124, 85, 139, 22, 251, 230, 32, 222, 96, 135, 62, 255, 167, 222, 37, 35, 0,
        176, 226, 51, 233, 101, 9, 37, 80, 96, 181, 224, 110,
    ];

    // Start with a known Stellar address string
    let addr_str = "GDQMHID4KWFRN67GEDPGBBZ676T54JJDACYOEM7JMUESKUDAWXQG5Z6Y";

    // Parse the address string into an Address
    let addr: Address = Address::from_str(&e, addr_str);

    // Convert to bytes
    let pk_bytes = address_to_ed25519_pk_bytes(&e, &addr);
    assert_eq!(pk_bytes.to_array(), raw_pk);

    // Convert back to address
    let addr_roundtrip = address_from_ed25519_pk_bytes(&e, &pk_bytes);

    // Verify roundtrip matches
    assert_eq!(addr_roundtrip.to_string(), addr.to_string());
}

#[test]
#[should_panic]
fn contract_address_rejected_for_ed25519_extraction() {
    let e = Env::default();
    let hash = BytesN::<32>::from_array(&e, &[7; 32]);
    let contract = Address::from_payload(&e, AddressPayload::ContractIdHash(hash));

    address_to_ed25519_pk_bytes(&e, &contract);
}
