use alloc::vec::Vec;
use pinocchio::Address;

use crate::traits::{EventDiscriminator, EventDiscriminators, EventSerialize};

/// Emitted when an escrow gets its settlement terms.
pub struct SettlementSetEvent {
    pub escrow: Address,
    pub beneficiary: Address,
    pub deposit_deadline: i64,
    pub escape_time: i64,
}

impl EventDiscriminator for SettlementSetEvent {
    const DISCRIMINATOR: u8 = EventDiscriminators::SettlementSet as u8;
}

impl EventSerialize for SettlementSetEvent {
    #[inline(always)]
    fn to_bytes_inner(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(Self::DATA_LEN);
        data.extend_from_slice(self.escrow.as_ref());
        data.extend_from_slice(self.beneficiary.as_ref());
        data.extend_from_slice(&self.deposit_deadline.to_le_bytes());
        data.extend_from_slice(&self.escape_time.to_le_bytes());
        data
    }
}

impl SettlementSetEvent {
    pub const DATA_LEN: usize = 32 + 32 + 8 + 8; // escrow + beneficiary + deposit_deadline + escape_time

    #[inline(always)]
    pub fn new(escrow: Address, beneficiary: Address, deposit_deadline: i64, escape_time: i64) -> Self {
        Self { escrow, beneficiary, deposit_deadline, escape_time }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::EVENT_IX_TAG_LE;
    use crate::traits::EVENT_DISCRIMINATOR_LEN;

    #[test]
    fn test_settlement_set_event_to_bytes() {
        let escrow = Address::new_from_array([1u8; 32]);
        let beneficiary = Address::new_from_array([2u8; 32]);
        let event = SettlementSetEvent::new(escrow, beneficiary, 1_000, 2_000);

        let bytes = event.to_bytes();
        assert_eq!(bytes.len(), EVENT_DISCRIMINATOR_LEN + SettlementSetEvent::DATA_LEN);
        assert_eq!(&bytes[..8], EVENT_IX_TAG_LE);
        assert_eq!(bytes[8], EventDiscriminators::SettlementSet as u8);
        assert_eq!(&bytes[9..41], escrow.as_ref());
        assert_eq!(&bytes[41..73], beneficiary.as_ref());
        assert_eq!(&bytes[73..81], &1_000i64.to_le_bytes());
        assert_eq!(&bytes[81..89], &2_000i64.to_le_bytes());
    }
}
