use pinocchio::error::ProgramError;

use crate::traits::InstructionData;

/// Instruction data for Refund
///
/// Everything comes from the receipt and the settlement terms, so there is no data.
pub struct RefundData {}

impl<'a> TryFrom<&'a [u8]> for RefundData {
    type Error = ProgramError;

    #[inline(always)]
    fn try_from(_data: &'a [u8]) -> Result<Self, Self::Error> {
        Ok(Self {})
    }
}

impl<'a> InstructionData<'a> for RefundData {
    const LEN: usize = 0;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_refund_data_try_from_empty() {
        let data: [u8; 0] = [];
        assert!(RefundData::try_from(&data[..]).is_ok());
    }
}
