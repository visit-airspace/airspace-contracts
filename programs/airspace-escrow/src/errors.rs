use pinocchio::error::ProgramError;
use thiserror::Error;

/// Errors that may be returned by the Escrow Program.
#[derive(Clone, Debug, Eq, PartialEq, Error)]
pub enum EscrowProgramError {
    /// (0) Escrow ID invalid or does not respect rules
    #[error("Escrow ID invalid or does not respect rules")]
    InvalidEscrowId,

    /// (1) Admin invalid or does not match escrow admin
    #[error("Admin invalid or does not match escrow admin")]
    InvalidAdmin,

    /// (2) Event authority PDA is invalid
    #[error("Event authority PDA is invalid")]
    InvalidEventAuthority,

    /// (3) Timelock has not expired yet
    #[error("Timelock has not expired yet")]
    TimelockNotExpired,

    /// (4) External hook rejected the operation
    #[error("External hook rejected the operation")]
    HookRejected,

    /// (5) Withdrawer does not match receipt depositor
    #[error("Withdrawer does not match receipt depositor")]
    InvalidWithdrawer,

    /// (6) Receipt escrow does not match escrow
    #[error("Receipt escrow does not match escrow")]
    InvalidReceiptEscrow,

    /// (7) Hook program mismatch
    #[error("Hook program mismatch")]
    HookProgramMismatch,

    /// (8) Mint is not allowed for this escrow
    #[error("Mint is not allowed for this escrow")]
    MintNotAllowed,

    /// (9) Mint has PermanentDelegate extension which is not allowed
    #[error("Mint has PermanentDelegate extension which is not allowed")]
    PermanentDelegateNotAllowed,

    /// (10) Mint has NonTransferable extension which is not allowed
    #[error("Mint has NonTransferable extension which is not allowed")]
    NonTransferableNotAllowed,

    /// (11) Mint has Pausable extension which is not allowed
    #[error("Mint has Pausable extension which is not allowed")]
    PausableNotAllowed,

    /// (12) Token extension already blocked
    #[error("Token extension already blocked")]
    TokenExtensionAlreadyBlocked,

    /// (13) Zero deposit amount
    #[error("Zero deposit amount")]
    ZeroDepositAmount,

    /// (14) Arbiter signer is missing or does not match
    #[error("Arbiter signer is missing or does not match")]
    InvalidArbiter,

    /// (15) Token extension is not currently blocked
    #[error("Token extension is not currently blocked")]
    TokenExtensionNotBlocked,

    /// (16) Escrow is immutable and cannot be modified
    #[error("Escrow is immutable and cannot be modified")]
    EscrowImmutable,

    /// (17) The escrow already has settlement terms, which never change
    #[error("The escrow already has settlement terms")]
    SettlementAlreadySet,

    /// (18) The escape time is less than 7 days after the deposit deadline
    #[error("Escape time must be at least 7 days after the deposit deadline")]
    InvalidSettlementTerms,

    /// (19) The deposit deadline of the settlement terms has passed
    #[error("The deposit deadline has passed")]
    DepositDeadlinePassed,

    /// (20) The escape time of the settlement terms has not been reached
    #[error("The escape time has not been reached")]
    EscapeTimeNotReached,

    /// (21) The escrow has no settlement terms
    #[error("The escrow has no settlement terms")]
    SettlementNotSet,

    /// (22) Settlement terms cannot be removed
    #[error("Settlement terms cannot be removed")]
    SettlementTermsPermanent,

    /// (23) The token account is not the associated token account of the only allowed recipient
    #[error("Recipient token account is not the allowed recipient's associated token account")]
    InvalidPayoutRecipient,

    /// (24) The rent recipient is not the depositor who paid for the receipt
    #[error("Rent recipient must be the depositor of the receipt")]
    InvalidRentRecipient,
}

impl From<EscrowProgramError> for ProgramError {
    fn from(e: EscrowProgramError) -> Self {
        ProgramError::Custom(e as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_conversion() {
        let error: ProgramError = EscrowProgramError::InvalidEscrowId.into();
        assert_eq!(error, ProgramError::Custom(0));

        let error: ProgramError = EscrowProgramError::InvalidAdmin.into();
        assert_eq!(error, ProgramError::Custom(1));

        let error: ProgramError = EscrowProgramError::InvalidEventAuthority.into();
        assert_eq!(error, ProgramError::Custom(2));

        let error: ProgramError = EscrowProgramError::TimelockNotExpired.into();
        assert_eq!(error, ProgramError::Custom(3));

        let error: ProgramError = EscrowProgramError::HookRejected.into();
        assert_eq!(error, ProgramError::Custom(4));

        let error: ProgramError = EscrowProgramError::InvalidWithdrawer.into();
        assert_eq!(error, ProgramError::Custom(5));

        let error: ProgramError = EscrowProgramError::EscrowImmutable.into();
        assert_eq!(error, ProgramError::Custom(16));

        let error: ProgramError = EscrowProgramError::SettlementAlreadySet.into();
        assert_eq!(error, ProgramError::Custom(17));

        let error: ProgramError = EscrowProgramError::InvalidRentRecipient.into();
        assert_eq!(error, ProgramError::Custom(24));
    }
}
