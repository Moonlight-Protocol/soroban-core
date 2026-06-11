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
pub const HELPER_ERROR_RANGE_START: u32 = 4_000;
pub const HELPER_ERROR_RANGE_END: u32 = 4_099;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum MoonlightError {
    // Authorization errors: 1000-1099.
    /// An authorization payload or contract argument could not be decoded into the expected shape.
    BadArg = 1_000,
    /// A value used during authorization matched a type family but not one of the supported variants.
    UnexpectedVariant = 1_001,
    /// A required signature for an authorization signer was not provided.
    MissingSignature = 1_002,
    /// More signatures were provided than the authorization context accepts.
    ExtraSignature = 1_003,
    /// A signature entry could not be parsed as a supported authorization signature shape.
    InvalidSignatureFormat = 1_004,
    /// A signature entry was well-formed but used a signature format this protocol does not support.
    UnsupportedSignatureFormat = 1_005,
    /// A signature or authorization context was produced for a different contract than the one being checked.
    MismatchedContract = 1_006,
    /// A signer kind is not supported by this authorization path.
    UnsupportedSigner = 1_007,
    /// Authorization was requested without any conditions to evaluate.
    NoConditions = 1_008,
    /// The authorization context did not match the invocation shape expected by the checker.
    UnexpectedContext = 1_009,
    /// A signature was valid structurally but expired before the current ledger.
    SignatureExpired = 1_010,
    /// The configured provider-signature threshold was not met.
    ProviderThresholdNotMet = 1_011,
    /// The provider account is already registered.
    ProviderAlreadyRegistered = 1_012,
    /// The provider account is not registered.
    ProviderNotRegistered = 1_013,

    // UTXO Module errors: 2000-2099.
    /// A UTXO creation attempted to write an output identifier that already exists.
    UtxoAlreadyExists = 2_000,
    /// A UTXO spend referenced an output identifier that does not exist.
    UtxoDoesNotExist = 2_001,
    /// A UTXO spend referenced an output that has already been spent.
    UtxoAlreadySpent = 2_002,
    /// The transaction bundle does not balance its inputs, deposits, creates, spends, and withdrawals.
    UnbalancedBundle = 2_003,
    /// A UTXO creation amount must be greater than zero.
    InvalidCreateAmount = 2_004,
    /// The same UTXO identifier appears more than once in the create set.
    RepeatedCreateUtxo = 2_005,
    /// The same UTXO identifier appears more than once in the spend set.
    RepeatedSpendUtxo = 2_006,
    /// A requested UTXO could not be found in storage.
    UtxoNotFound = 2_007,
    /// The UTXO module cannot authorize transactions because no authorization contract is configured.
    AuthContractNotSet = 2_008,
    /// A UTXO metadata record points to a slot outside its drawer bitmap range.
    InvalidDrawerSlot = 2_009,

    // Privacy channel errors: 3000-3099.
    /// The same account appears more than once in the deposit list.
    RepeatedAccountForDeposit = 3_000,
    /// The same account appears more than once in the withdraw list.
    RepeatedAccountForWithdraw = 3_001,
    /// A single account has conflicting deposit, withdraw, or condition requirements in the bundle.
    ConflictingConditionsForAccount = 3_002,
    /// An amount calculation exceeded the maximum supported integer value.
    AmountOverflow = 3_003,
    /// The bundle contains conditions that cannot all be satisfied together.
    BundleHasConflictingConditions = 3_004,
    /// An amount calculation went below the minimum supported integer value.
    AmountUnderflow = 3_005,
    /// An executed create/withdraw effect is not covered by an owner-signed condition,
    /// or an owner-signed create/withdraw condition is not executed by the bundle.
    UnauthorizedOperation = 3_006,

    // Helper errors: 4000-4099.
    /// An address payload was expected to be an Ed25519 account address but was not.
    NotEd25519AccountAddress = 4_000,
    /// The address payload type is not supported by this helper.
    UnsupportedAddressPayload = 4_001,
}

pub use MoonlightError as Error;

impl MoonlightError {
    pub const fn code(self) -> u32 {
        self as u32
    }
}
