use crate::{Error, MoonlightError};

#[test]
fn exposes_error_alias_for_contract_imports() {
    assert_eq!(Error::BadArg.code(), MoonlightError::BadArg.code());
}

#[test]
fn keeps_auth_errors_in_their_reserved_range() {
    for code in [
        Error::BadArg.code(),
        Error::UnexpectedVariant.code(),
        Error::MissingSignature.code(),
        Error::ExtraSignature.code(),
        Error::InvalidSignatureFormat.code(),
        Error::UnsupportedSignatureFormat.code(),
        Error::MismatchedContract.code(),
        Error::UnsupportedSigner.code(),
        Error::NoConditions.code(),
        Error::UnexpectedContext.code(),
        Error::SignatureExpired.code(),
        Error::ProviderThresholdNotMet.code(),
        Error::ProviderAlreadyRegistered.code(),
        Error::ProviderNotRegistered.code(),
    ] {
        assert!((1_000..=1_099).contains(&code));
    }
}

#[test]
fn keeps_utxo_errors_in_their_reserved_range() {
    for code in [
        Error::UtxoAlreadyExists.code(),
        Error::UtxoDoesNotExist.code(),
        Error::UtxoAlreadySpent.code(),
        Error::UnbalancedBundle.code(),
        Error::InvalidCreateAmount.code(),
        Error::RepeatedCreateUtxo.code(),
        Error::RepeatedSpendUtxo.code(),
        Error::UtxoNotFound.code(),
        Error::AuthContractNotSet.code(),
        Error::InvalidDrawerSlot.code(),
    ] {
        assert!((2_000..=2_099).contains(&code));
    }
}

#[test]
fn keeps_channel_errors_in_their_reserved_range() {
    for code in [
        Error::RepeatedAccountForDeposit.code(),
        Error::RepeatedAccountForWithdraw.code(),
        Error::ConflictingConditionsForAccount.code(),
        Error::AmountOverflow.code(),
        Error::BundleHasConflictingConditions.code(),
        Error::AmountUnderflow.code(),
    ] {
        assert!((3_000..=3_099).contains(&code));
    }
}

#[test]
fn keeps_helper_errors_in_their_reserved_range() {
    for code in [
        Error::NotEd25519AccountAddress.code(),
        Error::UnsupportedAddressPayload.code(),
    ] {
        assert!((4_000..=4_099).contains(&code));
    }
}
