use pinocchio::{
    account::AccountView,
    error::ProgramError,
    sysvars::{clock::Clock, Sysvar},
    Address, ProgramResult,
};

use crate::{
    errors::DepositProgramError,
    events::EventKind,
    instructions::payout::Payout,
    state::BidState,
    utils::{
        load_auction, load_bid, require_empty, verify_readonly, verify_recipient, verify_signer, verify_vault,
        verify_writable,
    },
};

/// Processes the Escape instruction.
///
/// A bidder takes back the deposit of a bid that is still `deposited`, from the escape time on, and
/// the bid closes. A `winner` or `paid` bid cannot escape, so only Settle and Forfeit finish it.
///
/// # Account Layout
/// 0. `[writable, signer]` bidder - Must be the bid's bidder, receives the rent of the bid
/// 1. `[]` auction
/// 2. `[writable]` bid
/// 3. `[writable]` vault
/// 4. `[writable]` bidder_token_account - Associated token account of the bidder for the mint
/// 5. `[]` mint
/// 6. `[]` token_program
/// 7. `[]` event_authority
/// 8. `[]` program
pub fn process_escape(program_id: &Address, accounts: &[AccountView], instruction_data: &[u8]) -> ProgramResult {
    let [bidder, auction, bid, vault, bidder_token_account, mint, token_program, event_authority, current_program] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    require_empty(instruction_data)?;

    verify_signer(bidder, true)?;
    verify_readonly(auction)?;
    verify_writable(bid, true)?;
    verify_writable(vault, true)?;
    verify_writable(bidder_token_account, true)?;
    verify_readonly(mint)?;

    let auction_state = load_auction(program_id, auction)?;
    let bid_state = load_bid(program_id, auction, bid)?;
    if bidder.address() != &bid_state.bidder {
        return Err(DepositProgramError::InvalidBidder.into());
    }
    if bid_state.state != BidState::Deposited {
        return Err(DepositProgramError::InvalidBidState.into());
    }
    if Clock::get()?.unix_timestamp < auction_state.escape_time {
        return Err(DepositProgramError::EscapeTimeNotReached.into());
    }
    verify_vault(&auction_state, vault, mint, token_program)?;
    verify_recipient(bidder_token_account, &bid_state.bidder, mint, token_program)?;

    let payout = Payout {
        program_id,
        auction,
        auction_state: &auction_state,
        bid,
        bid_state: &bid_state,
        vault,
        mint,
        token_program,
        bidder,
        event_authority,
        current_program,
    };
    payout.verify_bidder()?;
    payout.pay_and_close(bidder_token_account, bid_state.deposit, EventKind::BidEscaped)
}
