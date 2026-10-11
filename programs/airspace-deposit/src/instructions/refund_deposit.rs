use pinocchio::{account::AccountView, error::ProgramError, Address, ProgramResult};

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

/// Processes the RefundDeposit instruction.
///
/// The admin returns a `deposited` bid's whole deposit to the bidder and closes the bid. It pays no
/// one but the bidder, and a bid that was declared the winner is no longer `deposited`.
///
/// # Account Layout
/// 0. `[signer]` admin
/// 1. `[writable]` bidder - Must be the bid's bidder, receives the rent of the bid
/// 2. `[]` auction
/// 3. `[writable]` bid
/// 4. `[writable]` vault
/// 5. `[writable]` bidder_token_account - Associated token account of the bidder for the mint
/// 6. `[]` mint
/// 7. `[]` token_program
/// 8. `[]` event_authority
/// 9. `[]` program
pub fn process_refund_deposit(
    program_id: &Address,
    accounts: &[AccountView],
    instruction_data: &[u8],
) -> ProgramResult {
    let [admin, bidder, auction, bid, vault, bidder_token_account, mint, token_program, event_authority, current_program] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    require_empty(instruction_data)?;

    verify_signer(admin, false)?;
    verify_writable(bidder, true)?;
    verify_readonly(auction)?;
    verify_writable(bid, true)?;
    verify_writable(vault, true)?;
    verify_writable(bidder_token_account, true)?;
    verify_readonly(mint)?;

    let auction_state = load_auction(program_id, auction)?;
    if admin.address() != &auction_state.admin {
        return Err(DepositProgramError::InvalidAdmin.into());
    }
    let bid_state = load_bid(program_id, auction, bid)?;
    if bid_state.state != BidState::Deposited {
        return Err(DepositProgramError::InvalidBidState.into());
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
    payout.pay_and_close(bidder_token_account, bid_state.deposit, EventKind::DepositRefunded)
}
