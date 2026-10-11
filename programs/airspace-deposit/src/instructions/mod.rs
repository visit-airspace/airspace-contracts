//! One module per instruction. Each instruction starts with a one byte discriminator.

pub mod create_auction;
pub mod declare_winner;
pub mod emit_event;
pub mod escape;
pub mod forfeit;
pub mod pay_remainder;
pub mod payout;
pub mod place_deposit;
pub mod refund_deposit;
pub mod settle;

pub use create_auction::process_create_auction;
pub use declare_winner::process_declare_winner;
pub use emit_event::process_emit_event;
pub use escape::process_escape;
pub use forfeit::process_forfeit;
pub use pay_remainder::process_pay_remainder;
pub use place_deposit::process_place_deposit;
pub use refund_deposit::process_refund_deposit;
pub use settle::process_settle;

use pinocchio::error::ProgramError;

/// Instruction discriminators.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum DepositInstruction {
    CreateAuction = 0,
    PlaceDeposit = 1,
    RefundDeposit = 2,
    DeclareWinner = 3,
    PayRemainder = 4,
    Settle = 5,
    Forfeit = 6,
    Escape = 7,
    EmitEvent = 228,
}

impl TryFrom<u8> for DepositInstruction {
    type Error = ProgramError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::CreateAuction),
            1 => Ok(Self::PlaceDeposit),
            2 => Ok(Self::RefundDeposit),
            3 => Ok(Self::DeclareWinner),
            4 => Ok(Self::PayRemainder),
            5 => Ok(Self::Settle),
            6 => Ok(Self::Forfeit),
            7 => Ok(Self::Escape),
            228 => Ok(Self::EmitEvent),
            _ => Err(ProgramError::InvalidInstructionData),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discriminators_round_trip() {
        for value in [0u8, 1, 2, 3, 4, 5, 6, 7, 228] {
            assert_eq!(DepositInstruction::try_from(value).unwrap() as u8, value);
        }
    }

    #[test]
    fn test_unknown_discriminator_is_refused() {
        assert!(DepositInstruction::try_from(8).is_err());
        assert!(DepositInstruction::try_from(255).is_err());
    }
}
