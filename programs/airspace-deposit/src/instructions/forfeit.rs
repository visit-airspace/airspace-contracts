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
    utils::{load_auction, load_bid, require_empty, verify_readonly, verify_treasury, verify_vault, verify_writable},
};

/// Processes the Forfeit instruction.
///
/// Anyone may send it once a `winner` bid's remainder deadline has passed without payment. It moves
/// that bid's deposit from the vault to the treasury and closes the bid, with the rent going to the
/// bidder. The deposit is all the program ever keeps from a bidder who did not pay in full.
///
/// # Account Layout
/// 0. `[writable]` bidder - Must be the bid's bidder, receives the rent of the bid
/// 1. `[]` auction
/// 2. `[writable]` bid
/// 3. `[writable]` vault
/// 4. `[writable]` treasury - The auction's treasury token account
/// 5. `[]` mint
/// 6. `[]` token_program
/// 7. `[]` event_authority
/// 8. `[]` program
pub fn process_forfeit(program_id: &Address, accounts: &[AccountView], instruction_data: &[u8]) -> ProgramResult {
    let [bidder, auction, bid, vault, treasury, mint, token_program, event_authority, current_program] = accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    require_empty(instruction_data)?;

    verify_writable(bidder, true)?;
    verify_readonly(auction)?;
    verify_writable(bid, true)?;
    verify_writable(vault, true)?;
    verify_writable(treasury, true)?;
    verify_readonly(mint)?;

    let auction_state = load_auction(program_id, auction)?;
    let bid_state = load_bid(program_id, auction, bid)?;
    if bid_state.state != BidState::Winner {
        return Err(DepositProgramError::InvalidBidState.into());
    }
    if Clock::get()?.unix_timestamp <= bid_state.remainder_deadline {
        return Err(DepositProgramError::RemainderDeadlineNotPassed.into());
    }
    verify_vault(&auction_state, vault, mint, token_program)?;
    verify_treasury(&auction_state, treasury, token_program)?;

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
    payout.pay_and_close(treasury, bid_state.deposit, EventKind::BidForfeited)
}
