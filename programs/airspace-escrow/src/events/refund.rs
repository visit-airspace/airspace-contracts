use alloc::vec::Vec;
use pinocchio::Address;

use crate::traits::{EventDiscriminator, EventDiscriminators, EventSerialize};

/// Emitted when the escrow admin refund a receipt: the whole amount moved to `recipient`.
pub struct RefundEvent {
    pub escrow: Address,
    pub receipt: Address,
    pub depositor: Address,
    pub recipient: Address,
    pub mint: Address,
    pub amount: u64,
}

impl EventDiscriminator for RefundEvent {
    const DISCRIMINATOR: u8 = EventDiscriminators::Refund as u8;
}

impl EventSerialize for RefundEvent {
    #[inline(always)]
    fn to_bytes_inner(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(Self::DATA_LEN);
        data.extend_from_slice(self.escrow.as_ref());
        data.extend_from_slice(self.receipt.as_ref());
        data.extend_from_slice(self.depositor.as_ref());
        data.extend_from_slice(self.recipient.as_ref());
        data.extend_from_slice(self.mint.as_ref());
        data.extend_from_slice(&self.amount.to_le_bytes());
        data
    }
}

impl RefundEvent {
    pub const DATA_LEN: usize = 32 + 32 + 32 + 32 + 32 + 8; // escrow + receipt + depositor + recipient + mint + amount

    #[inline(always)]
    pub fn new(
        escrow: Address,
        receipt: Address,
        depositor: Address,
        recipient: Address,
        mint: Address,
        amount: u64,
    ) -> Self {
        Self { escrow, receipt, depositor, recipient, mint, amount }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::EVENT_IX_TAG_LE;
    use crate::traits::EVENT_DISCRIMINATOR_LEN;

    #[test]
    fn test_refund_event_to_bytes() {
        let event = RefundEvent::new(
            Address::new_from_array([1u8; 32]),
            Address::new_from_array([2u8; 32]),
            Address::new_from_array([3u8; 32]),
            Address::new_from_array([4u8; 32]),
            Address::new_from_array([5u8; 32]),
            5_000,
        );

        let bytes = event.to_bytes();
        assert_eq!(bytes.len(), EVENT_DISCRIMINATOR_LEN + RefundEvent::DATA_LEN);
        assert_eq!(&bytes[..8], EVENT_IX_TAG_LE);
        assert_eq!(bytes[8], EventDiscriminators::Refund as u8);
        assert_eq!(&bytes[9..41], &[1u8; 32]);
        assert_eq!(&bytes[41..73], &[2u8; 32]);
        assert_eq!(&bytes[73..105], &[3u8; 32]);
        assert_eq!(&bytes[105..137], &[4u8; 32]);
        assert_eq!(&bytes[137..169], &[5u8; 32]);
        assert_eq!(&bytes[169..177], &5_000u64.to_le_bytes());
    }
}
