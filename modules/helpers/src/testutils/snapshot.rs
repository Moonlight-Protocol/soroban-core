use core::str::FromStr;

use soroban_sdk::Env;

use crate::testutils::keys::Ed25519Account;
use stellar_strkey::ed25519;

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
    Env::from_ledger_snapshot_file(snapshot_path)
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
            &pk_bytes_from_string("GAV2OCUEI3GGZFPTBJDMUGIOKL6IEIW2PYATXVQAYWXEWKDDI573KC3D"),
            &sk_bytes_from_string(
                "5ca56b75f944da646cade355ba172fb2a2aa18aaa07d5455d097e773301fa8c5",
            ),
        ),
        Ed25519Account::from_keys(
            &e,
            &pk_bytes_from_string("GAUCMJZ7RFUVPMO3SYTFSE5DY67DPJJ5QXVGBTKRDGRSUNKZQXM56RVY"),
            &sk_bytes_from_string(
                "63a9d5559bae437c2dba445f241350ca15781c83ab08ff8dded7c6f166911b83",
            ),
        ),
        Ed25519Account::from_keys(
            &e,
            &pk_bytes_from_string("GBAAMZHIZ2TABWWE7WG2YODR2VCYZX6653MN77KAWPDUQT7MFRPRMMJA"),
            &sk_bytes_from_string(
                "4d00ca9c353175261013b7c12dabb0f7b9aef720d4fb3413a7efb21a31b6802c",
            ),
        ),
        Ed25519Account::from_keys(
            &e,
            &pk_bytes_from_string("GDFTOBI4RUPVUUTQ2DPDHSJEJTOT76EJCP4JWTXUA35PDGUPKHYRPKNJ"),
            &sk_bytes_from_string(
                "3fb9d9a7837d6cfe7f7fdce6c384e0a3b3f875fc48de3ee1b7b4e07c5b31840b",
            ),
        ),
        Ed25519Account::from_keys(
            &e,
            &pk_bytes_from_string("GBZJ7QK5CVELYMUGHNSMVTWJ4RW3DH7GL55C65ZFU44YK2K5NRXBH7PW"),
            &sk_bytes_from_string(
                "829fbfa26b788b2f2e0258d6f7b06377afbe976c46d8f0ecabba5ea84e109574",
            ),
        ),
    )
}

fn pk_bytes_from_string(s: &str) -> [u8; 32] {
    ed25519::PublicKey::from_str(s)
        .unwrap_or_else(|_| panic!("Invalid user_a public key"))
        .0
}

fn sk_bytes_from_string(s: &str) -> [u8; 32] {
    hex_to_bytes(s)
}
