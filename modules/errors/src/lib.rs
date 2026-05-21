#![no_std]

use soroban_sdk::contracterror;

#[cfg(test)]
pub mod test;

pub const AUTH_ERROR_RANGE_START: u32 = 1_000;
pub const AUTH_ERROR_RANGE_END: u32 = 1_099;
pub const UTXO_ERROR_RANGE_START: u32 = 2_000;
pub const UTXO_ERROR_RANGE_END: u32 = 2_099;
pub const CHANNEL_ERROR_RANGE_START: u32 = 3_000;
pub const CHANNEL_ERROR_RANGE_END: u32 = 3_099;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum MoonlightError {
    // Authorization errors: 1000-1099.
    BadArg = 1_000,
    UnexpectedVariant = 1_001,
    MissingSignature = 1_002,
    ExtraSignature = 1_003,
    InvalidSignatureFormat = 1_004,
    UnsupportedSignatureFormat = 1_005,
    MismatchedContract = 1_006,
    UnsupportedSigner = 1_007,
    NoConditions = 1_008,
    UnexpectedContext = 1_009,
    SignatureExpired = 1_010,
    ProviderThresholdNotMet = 1_011,

    // UTXO Module errors: 2000-2099.
    UtxoAlreadyExists = 2_000,
    UtxoDoesNotExist = 2_001,
    UtxoAlreadySpent = 2_002,
    UnbalancedBundle = 2_003,
    InvalidCreateAmount = 2_004,
    RepeatedCreateUtxo = 2_005,
    RepeatedSpendUtxo = 2_006,
    UtxoNotFound = 2_007,
    AuthContractNotSet = 2_008,

    // Privacy channel errors: 3000-3099.
    RepeatedAccountForDeposit = 3_000,
    RepeatedAccountForWithdraw = 3_001,
    ConflictingConditionsForAccount = 3_002,
    AmountOverflow = 3_003,
    BundleHasConflictingConditions = 3_004,
}

pub use MoonlightError as Error;

impl MoonlightError {
    pub const fn code(self) -> u32 {
        self as u32
    }
}
