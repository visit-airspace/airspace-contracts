use pinocchio::{
    account::AccountView,
    error::ProgramError,
    sysvars::{clock::Clock, Sysvar},
    Address, ProgramResult,
};

use crate::{
    errors::DepositProgramError,
    events::{BidEvent, EventKind},
    state::BidState,
    utils::{
        emit_event, load_auction, load_bid, require_empty, transfer_from_wallet, verify_readonly, verify_signer,
        verify_token_account, verify_vault, verify_writable, write_account,
    },
};

/// Processes the PayRemainder instruction.
///
/// The winning bidder moves exactly the bid amount minus the deposit into the vault, before the
/// remainder deadline. The program computes the amount, so there is no amount to get wrong.
///
/// # Account Layout
/// 0. `[signer]` bidder - Must be the bid's bidder
/// 1. `[]` auction
/// 2. `[writable]` bid
/// 3. `[writable]` vault
/// 4. `[writable]` bidder_token_account - The bidder's token account for the mint, the source
/// 5. `[]` mint
/// 6. `[]` token_program
/// 7. `[]` event_authority
/// 8. `[]` program
pub fn process_pay_remainder(program_id: &Address, accounts: &[AccountView], instruction_data: &[u8]) -> ProgramResult {
    let [bidder, auction, bid, vault, bidder_token_account, mint, token_program, event_authority, current_program] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    require_empty(instruction_data)?;

    verify_signer(bidder, false)?;
    verify_readonly(auction)?;
    verify_writable(bid, true)?;
    verify_writable(vault, true)?;
    verify_writable(bidder_token_account, true)?;
    verify_readonly(mint)?;

    let auction_state = load_auction(program_id, auction)?;
    let mut bid_state = load_bid(program_id, auction, bid)?;
    if bidder.address() != &bid_state.bidder {
        return Err(DepositProgramError::InvalidBidder.into());
    }
    if bid_state.state != BidState::Winner {
        return Err(DepositProgramError::InvalidBidState.into());
    }
    if &auction_state.winner_bid != bid.address() {
        return Err(DepositProgramError::InvalidBidState.into());
    }
    if Clock::get()?.unix_timestamp > bid_state.remainder_deadline {
        return Err(DepositProgramError::RemainderDeadlinePassed.into());
    }
    verify_vault(&auction_state, vault, mint, token_program)?;
    verify_token_account(bidder_token_account, mint.address(), token_program)?;

    let remainder = bid_state.remainder()?;
    bid_state.state = BidState::Paid;
    bid_state.remainder_paid = remainder;
    write_account(bid, &bid_state.to_bytes())?;
    transfer_from_wallet(bidder_token_account, vault, mint, bidder, token_program, remainder)?;

    let event = BidEvent {
        kind: EventKind::RemainderPaid,
        auction: *auction.address(),
        bid: *bid.address(),
        bidder: bid_state.bidder,
        recipient: auction_state.vault,
        amount: remainder,
    };
    emit_event(program_id, event_authority, current_program, &event.to_bytes())
}
