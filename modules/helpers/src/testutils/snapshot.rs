use soroban_sdk::{testutils::Ledger, Address, Env};

use crate::parser::address_to_ed25519_pk_bytes;
use crate::testutils::keys::Ed25519Account;

fn hex_to_bytes(hex_str: &str) -> [u8; 32] {
    let mut bytes = [0u8; 32];
    for i in 0..32 {
        let start = i * 2;
        let end = start + 2;
        bytes[i] = u8::from_str_radix(&hex_str[start..end], 16).unwrap();
    }
    bytes
}

pub fn get_env_with_g_accounts() -> Env {
    let snapshot_path = concat!(env!("CARGO_MANIFEST_DIR"), "/src/testutils/snapshot.json");
    let env = Env::from_ledger_snapshot_file(snapshot_path);
    env.ledger().set_protocol_version(25);
    env
}

pub fn get_snapshot_g_accounts(
    e: &Env,
) -> (
    Ed25519Account,
    Ed25519Account,
    Ed25519Account,
    Ed25519Account,
    Ed25519Account,
) {
    (
        Ed25519Account::from_keys(
            &e,
            &pk_bytes_from_string(
                &e,
                "GAV2OCUEI3GGZFPTBJDMUGIOKL6IEIW2PYATXVQAYWXEWKDDI573KC3D",
            ),
            &sk_bytes_from_string(
                "5ca56b75f944da646cade355ba172fb2a2aa18aaa07d5455d097e773301fa8c5",
            ),
        ),
        Ed25519Account::from_keys(
            &e,
            &pk_bytes_from_string(
                &e,
                "GAUCMJZ7RFUVPMO3SYTFSE5DY67DPJJ5QXVGBTKRDGRSUNKZQXM56RVY",
            ),
            &sk_bytes_from_string(
                "63a9d5559bae437c2dba445f241350ca15781c83ab08ff8dded7c6f166911b83",
            ),
        ),
        Ed25519Account::from_keys(
            &e,
            &pk_bytes_from_string(
                &e,
                "GBAAMZHIZ2TABWWE7WG2YODR2VCYZX6653MN77KAWPDUQT7MFRPRMMJA",
            ),
            &sk_bytes_from_string(
                "4d00ca9c353175261013b7c12dabb0f7b9aef720d4fb3413a7efb21a31b6802c",
            ),
        ),
        Ed25519Account::from_keys(
            &e,
            &pk_bytes_from_string(
                &e,
                "GDFTOBI4RUPVUUTQ2DPDHSJEJTOT76EJCP4JWTXUA35PDGUPKHYRPKNJ",
            ),
            &sk_bytes_from_string(
                "3fb9d9a7837d6cfe7f7fdce6c384e0a3b3f875fc48de3ee1b7b4e07c5b31840b",
            ),
        ),
        Ed25519Account::from_keys(
            &e,
            &pk_bytes_from_string(
                &e,
                "GBZJ7QK5CVELYMUGHNSMVTWJ4RW3DH7GL55C65ZFU44YK2K5NRXBH7PW",
            ),
            &sk_bytes_from_string(
                "829fbfa26b788b2f2e0258d6f7b06377afbe976c46d8f0ecabba5ea84e109574",
            ),
        ),
    )
}

fn pk_bytes_from_string(e: &Env, s: &str) -> [u8; 32] {
    let address = Address::from_str(e, s);
    address_to_ed25519_pk_bytes(e, &address).to_array()
}

fn sk_bytes_from_string(s: &str) -> [u8; 32] {
    hex_to_bytes(s)
}
