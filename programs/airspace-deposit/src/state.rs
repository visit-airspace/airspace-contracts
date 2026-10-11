//! Account state with fixed layouts. Every account starts with a one byte discriminator and a
//! one byte version, then its fields in the order of the structs, little endian, no padding.

use alloc::vec::Vec;
use pinocchio::{error::ProgramError, Address};

use crate::errors::DepositProgramError;

pub const AUCTION_DISCRIMINATOR: u8 = 1;
pub const BID_DISCRIMINATOR: u8 = 2;
pub const VERSION: u8 = 1;

pub const AUCTION_SEED: &[u8] = b"auction";
pub const BID_SEED: &[u8] = b"bid";

/// One dollar of the mint, which has 6 decimals.
pub const DOLLAR: u64 = 1_000_000;
pub const BASIS_POINTS: u64 = 10_000;
pub const MIN_DEPOSIT_BPS: u16 = 1000;
pub const MAX_DEPOSIT_BPS: u16 = 5000;
pub const DEPOSIT_BPS_STEP: u16 = 100;
pub const MIN_ESCAPE_MARGIN: i64 = 7 * 86_400;

/// The state of a bid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum BidState {
    /// The deposit is in the vault and the bid has not been declared the winner.
    Deposited = 0,
    /// The admin declared this bid the winner, and its remainder is due by the remainder deadline.
    Winner = 1,
    /// The winner paid the remainder, and the bid waits for Settle.
    Paid = 2,
}

impl TryFrom<u8> for BidState {
    type Error = ProgramError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(BidState::Deposited),
            1 => Ok(BidState::Winner),
            2 => Ok(BidState::Paid),
            _ => Err(ProgramError::InvalidAccountData),
        }
    }
}

/// Reads fields from a slice in order, and fails when the slice is too short.
struct Reader<'a> {
    data: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, len: usize) -> Result<&'a [u8], ProgramError> {
        assert!(self.at <= self.data.len(), "account reader cursor escaped its buffer");
        let start = self.at;
        let end = self.at.checked_add(len).ok_or(ProgramError::InvalidAccountData)?;
        let bytes = self.data.get(self.at..end).ok_or(ProgramError::InvalidAccountData)?;
        self.at = end;
        assert_eq!(self.at - start, bytes.len(), "account reader did not advance by the field width");
        Ok(bytes)
    }

    fn u8(&mut self) -> Result<u8, ProgramError> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, ProgramError> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().map_err(|_| ProgramError::InvalidAccountData)?))
    }

    fn u64(&mut self) -> Result<u64, ProgramError> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().map_err(|_| ProgramError::InvalidAccountData)?))
    }

    fn i64(&mut self) -> Result<i64, ProgramError> {
        Ok(i64::from_le_bytes(self.take(8)?.try_into().map_err(|_| ProgramError::InvalidAccountData)?))
    }

    fn address(&mut self) -> Result<Address, ProgramError> {
        let bytes: [u8; 32] = self.take(32)?.try_into().map_err(|_| ProgramError::InvalidAccountData)?;
        Ok(Address::new_from_array(bytes))
    }
}

/// Checks the discriminator and the version, and returns a reader of the fields.
fn open(data: &[u8], discriminator: u8, len: usize) -> Result<Reader<'_>, ProgramError> {
    assert!(len >= 2, "account layout must include a discriminator and version");
    assert_ne!(discriminator, 0, "zero is reserved for uninitialized accounts");
    if data.len() != len {
        return Err(ProgramError::InvalidAccountData);
    }
    if data[0] != discriminator {
        return Err(ProgramError::InvalidAccountData);
    }
    if data[1] != VERSION {
        return Err(ProgramError::InvalidAccountData);
    }
    Ok(Reader { data, at: 2 })
}

/// The terms of one auction, at seeds `["auction", admin, auction_id]`. They never change after
/// CreateAuction, except `winner_bid`, which DeclareWinner sets once.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Auction {
    pub bump: u8,
    pub auction_id: Address,
    pub admin: Address,
    pub mint: Address,
    pub treasury: Address,
    pub vault: Address,
    pub deposit_bps: u16,
    pub min_deposit: u64,
    pub bid_deadline: i64,
    pub payment_window: i64,
    pub escape_time: i64,
    /// The declared winning bid, or the zero address while there is none.
    pub winner_bid: Address,
}

impl Auction {
    pub const LEN: usize = 2 + 1 + 32 * 5 + 2 + 8 * 4 + 32;

    pub fn from_bytes(data: &[u8]) -> Result<Self, ProgramError> {
        let mut r = open(data, AUCTION_DISCRIMINATOR, Self::LEN)?;
        let auction = Self {
            bump: r.u8()?,
            auction_id: r.address()?,
            admin: r.address()?,
            mint: r.address()?,
            treasury: r.address()?,
            vault: r.address()?,
            deposit_bps: r.u16()?,
            min_deposit: r.u64()?,
            bid_deadline: r.i64()?,
            payment_window: r.i64()?,
            escape_time: r.i64()?,
            winner_bid: r.address()?,
        };
        assert_eq!(r.at, Self::LEN, "auction decoder did not consume its complete layout");
        Ok(auction)
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(Self::LEN);
        data.push(AUCTION_DISCRIMINATOR);
        data.push(VERSION);
        data.push(self.bump);
        data.extend_from_slice(self.auction_id.as_ref());
        data.extend_from_slice(self.admin.as_ref());
        data.extend_from_slice(self.mint.as_ref());
        data.extend_from_slice(self.treasury.as_ref());
        data.extend_from_slice(self.vault.as_ref());
        data.extend_from_slice(&self.deposit_bps.to_le_bytes());
        data.extend_from_slice(&self.min_deposit.to_le_bytes());
        data.extend_from_slice(&self.bid_deadline.to_le_bytes());
        data.extend_from_slice(&self.payment_window.to_le_bytes());
        data.extend_from_slice(&self.escape_time.to_le_bytes());
        data.extend_from_slice(self.winner_bid.as_ref());
        assert_eq!(data.len(), Self::LEN, "auction encoder disagrees with the account layout");
        data
    }

    /// Whether a winner was declared.
    pub fn has_winner(&self) -> bool {
        self.winner_bid != Address::new_from_array([0u8; 32])
    }

    /// Checks the terms that CreateAuction accepts.
    pub fn validate_terms(&self, now: i64) -> Result<(), ProgramError> {
        if self.deposit_bps < MIN_DEPOSIT_BPS {
            return Err(DepositProgramError::InvalidDepositShare.into());
        }
        if self.deposit_bps > MAX_DEPOSIT_BPS {
            return Err(DepositProgramError::InvalidDepositShare.into());
        }
        if !self.deposit_bps.is_multiple_of(DEPOSIT_BPS_STEP) {
            return Err(DepositProgramError::InvalidDepositShare.into());
        }
        if self.min_deposit == 0 {
            return Err(DepositProgramError::InvalidMinimumDeposit.into());
        }
        if !self.min_deposit.is_multiple_of(DOLLAR) {
            return Err(DepositProgramError::InvalidMinimumDeposit.into());
        }
        if self.bid_deadline <= now {
            return Err(DepositProgramError::InvalidAuctionTerms.into());
        }
        if self.payment_window <= 0 {
            return Err(DepositProgramError::InvalidAuctionTerms.into());
        }
        let earliest_escape = self
            .bid_deadline
            .checked_add(self.payment_window)
            .and_then(|t| t.checked_add(MIN_ESCAPE_MARGIN))
            .ok_or(DepositProgramError::ArithmeticOverflow)?;
        if self.escape_time < earliest_escape {
            return Err(DepositProgramError::InvalidAuctionTerms.into());
        }
        Ok(())
    }

    /// The deposit for a bid: the deposit share of the amount rounded up to a whole dollar, and
    /// never less than the minimum deposit. The amount must be a whole number of dollars and at
    /// least the minimum deposit.
    pub fn deposit_for(&self, amount: u64) -> Result<u64, ProgramError> {
        if amount == 0 {
            return Err(DepositProgramError::InvalidBidAmount.into());
        }
        if !amount.is_multiple_of(DOLLAR) {
            return Err(DepositProgramError::InvalidBidAmount.into());
        }
        if amount < self.min_deposit {
            return Err(DepositProgramError::InvalidBidAmount.into());
        }
        let share =
            (amount as u128).checked_mul(self.deposit_bps as u128).ok_or(DepositProgramError::ArithmeticOverflow)?
                / BASIS_POINTS as u128;
        let dollars = share.div_ceil(DOLLAR as u128);
        let rounded = dollars.checked_mul(DOLLAR as u128).ok_or(DepositProgramError::ArithmeticOverflow)?;
        let deposit = u64::try_from(rounded).map_err(|_| DepositProgramError::ArithmeticOverflow)?;
        let deposit = deposit.max(self.min_deposit).min(amount);
        assert!(deposit >= self.min_deposit, "deposit calculation violated the auction minimum");
        assert!(deposit <= amount, "deposit calculation exceeded the bid amount");
        Ok(deposit)
    }
}

/// One bid, at seeds `["bid", auction, bidder, bid_seed]`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bid {
    pub bump: u8,
    pub state: BidState,
    pub auction: Address,
    pub bidder: Address,
    pub bid_seed: Address,
    pub amount: u64,
    pub deposit: u64,
    pub remainder_paid: u64,
    /// The time by which the winner must pay, or 0 until the bid is declared the winner.
    pub remainder_deadline: i64,
}

impl Bid {
    pub const LEN: usize = 2 + 1 + 1 + 32 * 3 + 8 * 4;

    pub fn from_bytes(data: &[u8]) -> Result<Self, ProgramError> {
        let mut r = open(data, BID_DISCRIMINATOR, Self::LEN)?;
        let bid = Self {
            bump: r.u8()?,
            state: BidState::try_from(r.u8()?)?,
            auction: r.address()?,
            bidder: r.address()?,
            bid_seed: r.address()?,
            amount: r.u64()?,
            deposit: r.u64()?,
            remainder_paid: r.u64()?,
            remainder_deadline: r.i64()?,
        };
        assert_eq!(r.at, Self::LEN, "bid decoder did not consume its complete layout");
        Ok(bid)
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(Self::LEN);
        data.push(BID_DISCRIMINATOR);
        data.push(VERSION);
        data.push(self.bump);
        data.push(self.state as u8);
        data.extend_from_slice(self.auction.as_ref());
        data.extend_from_slice(self.bidder.as_ref());
        data.extend_from_slice(self.bid_seed.as_ref());
        data.extend_from_slice(&self.amount.to_le_bytes());
        data.extend_from_slice(&self.deposit.to_le_bytes());
        data.extend_from_slice(&self.remainder_paid.to_le_bytes());
        data.extend_from_slice(&self.remainder_deadline.to_le_bytes());
        assert_eq!(data.len(), Self::LEN, "bid encoder disagrees with the account layout");
        data
    }

    /// The part of the bid that the winner still has to pay.
    pub fn remainder(&self) -> Result<u64, ProgramError> {
        self.amount.checked_sub(self.deposit).ok_or_else(|| DepositProgramError::ArithmeticOverflow.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reader_failure_preserves_cursor() {
        let data = [1u8, 2];
        let mut reader = Reader { data: &data, at: 0 };
        assert_eq!(reader.take(3), Err(ProgramError::InvalidAccountData));
        assert_eq!(reader.at, 0);
        assert_eq!(reader.take(2).unwrap(), &data);
        assert_eq!(reader.take(1), Err(ProgramError::InvalidAccountData));
        assert_eq!(reader.at, data.len());
    }

    #[test]
    fn test_truncated_account_data_remains_recoverable() {
        let auction_data = auction(2000).to_bytes();
        for len in 0..Auction::LEN {
            assert_eq!(Auction::from_bytes(&auction_data[..len]), Err(ProgramError::InvalidAccountData));
        }
        let mut bid_data = [0u8; Bid::LEN];
        bid_data[0] = BID_DISCRIMINATOR;
        bid_data[1] = VERSION;
        for len in 0..Bid::LEN {
            assert_eq!(Bid::from_bytes(&bid_data[..len]), Err(ProgramError::InvalidAccountData));
        }
    }

    fn auction(deposit_bps: u16) -> Auction {
        Auction {
            bump: 255,
            auction_id: Address::new_from_array([1; 32]),
            admin: Address::new_from_array([2; 32]),
            mint: Address::new_from_array([3; 32]),
            treasury: Address::new_from_array([4; 32]),
            vault: Address::new_from_array([5; 32]),
            deposit_bps,
            min_deposit: 10 * DOLLAR,
            bid_deadline: 1_000,
            payment_window: 172_800,
            escape_time: 1_000 + 172_800 + MIN_ESCAPE_MARGIN,
            winner_bid: Address::new_from_array([0; 32]),
        }
    }

    #[test]
    fn test_auction_round_trip() {
        let a = auction(2000);
        let bytes = a.to_bytes();
        assert_eq!(bytes.len(), Auction::LEN);
        assert_eq!(Auction::from_bytes(&bytes).unwrap(), a);
    }

    #[test]
    fn test_bid_round_trip() {
        let bid = Bid {
            bump: 254,
            state: BidState::Winner,
            auction: Address::new_from_array([1; 32]),
            bidder: Address::new_from_array([2; 32]),
            bid_seed: Address::new_from_array([3; 32]),
            amount: 100 * DOLLAR,
            deposit: 20 * DOLLAR,
            remainder_paid: 0,
            remainder_deadline: 5,
        };
        let bytes = bid.to_bytes();
        assert_eq!(bytes.len(), Bid::LEN);
        assert_eq!(Bid::from_bytes(&bytes).unwrap(), bid);
    }

    #[test]
    fn test_wrong_discriminator_is_refused() {
        let mut bytes = auction(2000).to_bytes();
        bytes[0] = BID_DISCRIMINATOR;
        assert!(Auction::from_bytes(&bytes).is_err());
    }

    #[test]
    fn test_deposit_share_rounds_up_to_a_whole_dollar() {
        // 20 percent of 101 dollars is 20.2 dollars, which rounds up to 21
        assert_eq!(auction(2000).deposit_for(101 * DOLLAR).unwrap(), 21 * DOLLAR);
        // 20 percent of 100 dollars is exactly 20
        assert_eq!(auction(2000).deposit_for(100 * DOLLAR).unwrap(), 20 * DOLLAR);
    }

    #[test]
    fn test_deposit_is_never_below_the_minimum() {
        // 10 percent of 50 dollars is 5, below the minimum of 10
        assert_eq!(auction(1000).deposit_for(50 * DOLLAR).unwrap(), 10 * DOLLAR);
    }

    #[test]
    fn test_deposit_is_never_above_the_bid() {
        assert_eq!(auction(5000).deposit_for(10 * DOLLAR).unwrap(), 10 * DOLLAR);
    }

    #[test]
    fn test_bid_amount_rules() {
        let a = auction(2000);
        assert!(a.deposit_for(0).is_err());
        assert!(a.deposit_for(100 * DOLLAR + 1).is_err());
        assert!(a.deposit_for(9 * DOLLAR).is_err());
        assert!(a.deposit_for(10 * DOLLAR).is_ok());
    }

    #[test]
    fn test_deposit_share_rules() {
        for bps in [999, 1050, 5100, 0, 10_000] {
            assert!(auction(bps).validate_terms(0).is_err(), "{bps} must be refused");
        }
        for bps in [1000, 1500, 2000, 5000] {
            assert!(auction(bps).validate_terms(0).is_ok(), "{bps} must be accepted");
        }
    }

    #[test]
    fn test_escape_must_follow_the_payment_window_by_seven_days() {
        let mut a = auction(2000);
        a.escape_time -= 1;
        assert!(a.validate_terms(0).is_err());
    }

    #[test]
    fn test_bid_deadline_must_be_in_the_future() {
        assert!(auction(2000).validate_terms(1_000).is_err());
    }
}
