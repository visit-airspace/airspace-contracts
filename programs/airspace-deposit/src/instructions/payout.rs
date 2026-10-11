//! The part that RefundDeposit, Escape, Settle, and Forfeit share: pay one bid's money out of the
//! vault to one recipient, close the bid, return its rent to the bidder, and emit the event.

use pinocchio::{account::AccountView, error::ProgramError, Address, ProgramResult};

use crate::{
    errors::DepositProgramError,
    events::{BidEvent, EventKind},
    state::{Auction, Bid},
    utils::{close_pda_account, emit_event, transfer_from_vault},
};

/// Everything that a payout reads.
pub struct Payout<'a> {
    pub program_id: &'a Address,
    pub auction: &'a AccountView,
    pub auction_state: &'a Auction,
    pub bid: &'a AccountView,
    pub bid_state: &'a Bid,
    pub vault: &'a AccountView,
    pub mint: &'a AccountView,
    pub token_program: &'a AccountView,
    pub bidder: &'a AccountView,
    pub event_authority: &'a AccountView,
    pub current_program: &'a AccountView,
}

impl Payout<'_> {
    /// The bidder that receives the rent must be the bid's bidder.
    pub fn verify_bidder(&self) -> Result<(), ProgramError> {
        if self.bidder.address() != &self.bid_state.bidder {
            return Err(DepositProgramError::InvalidBidder.into());
        }
        Ok(())
    }

    /// Moves `amount` from the vault to `to`, closes the bid, and emits the event.
    pub fn pay_and_close(&self, to: &AccountView, amount: u64, kind: EventKind) -> ProgramResult {
        transfer_from_vault(self.auction, self.auction_state, self.vault, to, self.mint, self.token_program, amount)?;
        close_pda_account(self.bid, self.bidder)?;
        let event = BidEvent {
            kind,
            auction: *self.auction.address(),
            bid: *self.bid.address(),
            bidder: self.bid_state.bidder,
            recipient: *to.address(),
            amount,
        };
        emit_event(self.program_id, self.event_authority, self.current_program, &event.to_bytes())
    }
}
