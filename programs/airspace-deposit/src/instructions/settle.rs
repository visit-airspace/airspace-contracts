use pinocchio::{account::AccountView, error::ProgramError, Address, ProgramResult};

use crate::{
    errors::DepositProgramError,
    events::EventKind,
    instructions::payout::Payout,
    state::BidState,
    utils::{load_auction, load_bid, require_empty, verify_readonly, verify_treasury, verify_vault, verify_writable},
};

/// Processes the Settle instruction.
///
/// Anyone may send it. It moves a `paid` bid's whole amount from the vault to the auction's
/// treasury and closes the bid, with the rent going to the bidder. It needs no signer, because the
/// program checks the state and the recipient.
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
pub fn process_settle(program_id: &Address, accounts: &[AccountView], instruction_data: &[u8]) -> ProgramResult {
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
    if bid_state.state != BidState::Paid {
        return Err(DepositProgramError::InvalidBidState.into());
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
    payout.pay_and_close(treasury, bid_state.amount, EventKind::BidSettled)
}
