use pinocchio::error::ProgramError;

use crate::traits::InstructionData;

/// Instruction data for Settle
///
/// Everything comes from the receipt and the settlement terms, so there is no data.
pub struct SettleData {}

impl<'a> TryFrom<&'a [u8]> for SettleData {
    type Error = ProgramError;

    #[inline(always)]
    fn try_from(_data: &'a [u8]) -> Result<Self, Self::Error> {
        Ok(Self {})
    }
}

impl<'a> InstructionData<'a> for SettleData {
    const LEN: usize = 0;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_settle_data_try_from_empty() {
        let data: [u8; 0] = [];
        assert!(SettleData::try_from(&data[..]).is_ok());
    }
}
