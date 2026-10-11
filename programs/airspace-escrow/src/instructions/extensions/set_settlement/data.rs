use pinocchio::{error::ProgramError, Address};

use crate::{require_len, traits::InstructionData};

/// Instruction data for SetSettlement
///
/// # Layout
/// * `extensions_bump` (u8) - Bump for extensions PDA
/// * `beneficiary` (Address) - Owner of the token accounts that Settle pays
/// * `deposit_deadline` (i64) - Unix time after which Deposit is refused
/// * `escape_time` (i64) - Unix time from which a depositor can withdraw on their own
pub struct SetSettlementData {
    pub extensions_bump: u8,
    pub beneficiary: Address,
    pub deposit_deadline: i64,
    pub escape_time: i64,
}

impl<'a> TryFrom<&'a [u8]> for SetSettlementData {
    type Error = ProgramError;

    #[inline(always)]
    fn try_from(data: &'a [u8]) -> Result<Self, Self::Error> {
        require_len!(data, Self::LEN);

        Ok(Self {
            extensions_bump: data[0],
            beneficiary: Address::new_from_array(data[1..33].try_into().unwrap()),
            deposit_deadline: i64::from_le_bytes(data[33..41].try_into().unwrap()),
            escape_time: i64::from_le_bytes(data[41..49].try_into().unwrap()),
        })
    }
}

impl<'a> InstructionData<'a> for SetSettlementData {
    const LEN: usize = 1 + 32 + 8 + 8; // 49 bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_settlement_data_try_from_valid() {
        let mut data = [0u8; 49];
        data[0] = 254;
        data[1..33].copy_from_slice(&[9u8; 32]);
        data[33..41].copy_from_slice(&1_000i64.to_le_bytes());
        data[41..49].copy_from_slice(&700_000i64.to_le_bytes());

        let parsed = SetSettlementData::try_from(&data[..]).unwrap();
        assert_eq!(parsed.extensions_bump, 254);
        assert_eq!(parsed.beneficiary.as_ref(), &[9u8; 32]);
        assert_eq!(parsed.deposit_deadline, 1_000);
        assert_eq!(parsed.escape_time, 700_000);
    }

    #[test]
    fn test_set_settlement_data_try_from_short() {
        let data = [0u8; 48];
        assert!(matches!(SetSettlementData::try_from(&data[..]), Err(ProgramError::InvalidInstructionData)));
    }
}
