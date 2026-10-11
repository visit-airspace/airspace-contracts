//! Events. An event is the program calling itself through EmitEvent, so the event data lives in
//! the instruction data of an inner instruction, which an indexer can read without log truncation.
//! The data is the 8 byte event tag, one byte event number, then the fields of the event.

use alloc::vec::Vec;
use const_crypto::ed25519;
use pinocchio::Address;

pub const EVENT_AUTHORITY_SEED: &[u8] = b"event_authority";

// Anchor compatible tag: Sha256(anchor:event)[..8]
pub const EVENT_IX_TAG: u64 = 0x1d9acb512ea545e4;
pub const EVENT_IX_TAG_LE: &[u8] = EVENT_IX_TAG.to_le_bytes().as_slice();

/// Event authority PDA, which only signs the self call that carries an event.
pub mod event_authority_pda {
    use super::*;

    const EVENT_AUTHORITY_AND_BUMP: ([u8; 32], u8) =
        ed25519::derive_program_address(&[EVENT_AUTHORITY_SEED], crate::ID.as_array());

    pub const ID: Address = Address::new_from_array(EVENT_AUTHORITY_AND_BUMP.0);
    pub const BUMP: u8 = EVENT_AUTHORITY_AND_BUMP.1;
}

/// The event numbers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum EventKind {
    AuctionCreated = 0,
    DepositPlaced = 1,
    DepositRefunded = 2,
    WinnerDeclared = 3,
    RemainderPaid = 4,
    BidSettled = 5,
    BidForfeited = 6,
    BidEscaped = 7,
}

fn start(kind: EventKind, capacity: usize) -> Vec<u8> {
    let mut data = Vec::with_capacity(EVENT_IX_TAG_LE.len() + 1 + capacity);
    data.extend_from_slice(EVENT_IX_TAG_LE);
    data.push(kind as u8);
    assert_eq!(data.len(), 9, "event prefix must contain the tag and kind");
    assert_eq!(&data[..8], EVENT_IX_TAG_LE, "event prefix lost its self-CPI tag");
    assert_eq!(data[8], kind as u8, "event prefix contains the wrong kind");
    data
}

/// Emitted by CreateAuction.
pub struct AuctionCreatedEvent {
    pub auction: Address,
    pub admin: Address,
    pub mint: Address,
    pub treasury: Address,
    pub deposit_bps: u16,
    pub min_deposit: u64,
    pub bid_deadline: i64,
    pub payment_window: i64,
    pub escape_time: i64,
}

impl AuctionCreatedEvent {
    pub const DATA_LEN: usize = 32 * 4 + 2 + 8 * 4;

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut data = start(EventKind::AuctionCreated, Self::DATA_LEN);
        data.extend_from_slice(self.auction.as_ref());
        data.extend_from_slice(self.admin.as_ref());
        data.extend_from_slice(self.mint.as_ref());
        data.extend_from_slice(self.treasury.as_ref());
        data.extend_from_slice(&self.deposit_bps.to_le_bytes());
        data.extend_from_slice(&self.min_deposit.to_le_bytes());
        data.extend_from_slice(&self.bid_deadline.to_le_bytes());
        data.extend_from_slice(&self.payment_window.to_le_bytes());
        data.extend_from_slice(&self.escape_time.to_le_bytes());
        assert_eq!(data.len(), 9 + Self::DATA_LEN, "auction event encoder disagrees with its layout");
        assert_eq!(data[8], EventKind::AuctionCreated as u8, "auction event contains the wrong kind");
        data
    }
}

/// Emitted by every instruction that works on a bid. `recipient` is the token account that
/// received money, the vault for a deposit or a remainder, and the zero address where no money
/// moves, which is WinnerDeclared. `amount` is the amount that moved, and for WinnerDeclared it is
/// the bid amount.
pub struct BidEvent {
    pub kind: EventKind,
    pub auction: Address,
    pub bid: Address,
    pub bidder: Address,
    pub recipient: Address,
    pub amount: u64,
}

impl BidEvent {
    pub const DATA_LEN: usize = 32 * 4 + 8;

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut data = start(self.kind, Self::DATA_LEN);
        data.extend_from_slice(self.auction.as_ref());
        data.extend_from_slice(self.bid.as_ref());
        data.extend_from_slice(self.bidder.as_ref());
        data.extend_from_slice(self.recipient.as_ref());
        data.extend_from_slice(&self.amount.to_le_bytes());
        assert_eq!(data.len(), 9 + Self::DATA_LEN, "bid event encoder disagrees with its layout");
        assert_eq!(data[8], self.kind as u8, "bid event contains the wrong kind");
        data
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bid_event_layout() {
        let event = BidEvent {
            kind: EventKind::BidForfeited,
            auction: Address::new_from_array([1; 32]),
            bid: Address::new_from_array([2; 32]),
            bidder: Address::new_from_array([3; 32]),
            recipient: Address::new_from_array([4; 32]),
            amount: 7,
        };
        let bytes = event.to_bytes();
        assert_eq!(bytes.len(), 8 + 1 + BidEvent::DATA_LEN);
        assert_eq!(&bytes[..8], EVENT_IX_TAG_LE);
        assert_eq!(bytes[8], 6);
        assert_eq!(&bytes[bytes.len() - 8..], &7u64.to_le_bytes());
    }

    #[test]
    fn test_auction_created_layout() {
        let event = AuctionCreatedEvent {
            auction: Address::new_from_array([1; 32]),
            admin: Address::new_from_array([2; 32]),
            mint: Address::new_from_array([3; 32]),
            treasury: Address::new_from_array([4; 32]),
            deposit_bps: 2000,
            min_deposit: 10,
            bid_deadline: 1,
            payment_window: 2,
            escape_time: 3,
        };
        let bytes = event.to_bytes();
        assert_eq!(bytes.len(), 8 + 1 + AuctionCreatedEvent::DATA_LEN);
        assert_eq!(bytes[8], 0);
    }
}
