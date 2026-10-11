use pinocchio::error::ProgramError;
use thiserror::Error;

/// Errors that may be returned by the Deposit Program.
#[derive(Clone, Debug, Eq, PartialEq, Error)]
pub enum DepositProgramError {
    /// (0) Event authority PDA is invalid
    #[error("Event authority PDA is invalid")]
    InvalidEventAuthority,

    /// (1) The signer is not the admin of the auction
    #[error("Admin invalid or does not match auction admin")]
    InvalidAdmin,

    /// (2) The signer or the rent recipient is not the bidder of the bid
    #[error("Bidder does not match the bid")]
    InvalidBidder,

    /// (3) The deposit share is not between 1000 and 5000 basis points, in steps of 100
    #[error("Deposit share must be 1000 to 5000 basis points, in steps of 100")]
    InvalidDepositShare,

    /// (4) The minimum deposit is zero or not a whole number of dollars
    #[error("Minimum deposit must be a positive whole number of dollars")]
    InvalidMinimumDeposit,

    /// (5) The deadlines of the auction are not in the required order
    #[error("Auction deadlines are not in the required order")]
    InvalidAuctionTerms,

    /// (6) The bid deadline has passed
    #[error("The bid deadline has passed")]
    BidDeadlinePassed,

    /// (7) The bid deadline has not passed yet
    #[error("The bid deadline has not passed yet")]
    BidDeadlineNotReached,

    /// (8) The bid amount is zero, not a whole number of dollars, or below the minimum deposit
    #[error("Bid amount must be a whole number of dollars and at least the minimum deposit")]
    InvalidBidAmount,

    /// (9) The mint is not the auction's mint, or it does not have 6 decimals
    #[error("Mint does not match the auction or does not have 6 decimals")]
    InvalidMint,

    /// (10) The mint carries token extensions, which the program does not accept
    #[error("Mint has token extensions, which are not allowed")]
    MintHasExtensions,

    /// (11) The bid is not in the state that this instruction needs
    #[error("Bid is not in the state this instruction needs")]
    InvalidBidState,

    /// (12) The auction already has a declared winner
    #[error("The auction already has a declared winner")]
    WinnerAlreadyDeclared,

    /// (13) The payment window of a declaration would pass the escape time
    #[error("The payment window would pass the escape time")]
    PaymentWindowPassesEscape,

    /// (14) The remainder deadline has passed
    #[error("The remainder deadline has passed")]
    RemainderDeadlinePassed,

    /// (15) The remainder deadline has not passed yet
    #[error("The remainder deadline has not passed yet")]
    RemainderDeadlineNotPassed,

    /// (16) The escape time has not been reached
    #[error("The escape time has not been reached")]
    EscapeTimeNotReached,

    /// (17) The vault is not the auction's vault
    #[error("Vault does not match the auction")]
    InvalidVault,

    /// (18) The treasury token account is not the auction's treasury, or a new auction's treasury
    /// is not an associated token account without extensions
    #[error("Treasury token account does not match the auction or is not a plain associated token account")]
    InvalidTreasury,

    /// (19) The token account is not the associated token account of the only allowed recipient
    #[error("Token account is not the allowed recipient's associated token account")]
    InvalidRecipient,

    /// (20) The account does not match the seeds that it must have
    #[error("Account does not match its seeds")]
    InvalidSeeds,

    /// (21) An amount overflowed
    #[error("Arithmetic overflow")]
    ArithmeticOverflow,

    /// (22) The bid does not belong to the auction
    #[error("Bid does not belong to the auction")]
    InvalidBidAuction,
}

impl From<DepositProgramError> for ProgramError {
    fn from(e: DepositProgramError) -> Self {
        ProgramError::Custom(e as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_conversion() {
        let error: ProgramError = DepositProgramError::InvalidEventAuthority.into();
        assert_eq!(error, ProgramError::Custom(0));

        let error: ProgramError = DepositProgramError::InvalidBidAuction.into();
        assert_eq!(error, ProgramError::Custom(22));
    }
}
