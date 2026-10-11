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
        emit_event, load_auction, load_bid, require_empty, verify_readonly, verify_signer, verify_writable,
        write_account,
    },
};

/// Processes the DeclareWinner instruction.
///
/// The admin declares one `deposited` bid the winner, after the bid deadline, and only while the
/// payment window that starts now still ends before the escape time. It runs once per auction.
///
/// # Account Layout
/// 0. `[signer]` admin
/// 1. `[writable]` auction
/// 2. `[writable]` bid
/// 3. `[]` event_authority
/// 4. `[]` program
pub fn process_declare_winner(
    program_id: &Address,
    accounts: &[AccountView],
    instruction_data: &[u8],
) -> ProgramResult {
    let [admin, auction, bid, event_authority, current_program] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    require_empty(instruction_data)?;

    verify_signer(admin, false)?;
    verify_writable(auction, true)?;
    verify_writable(bid, true)?;
    verify_readonly(event_authority)?;

    let mut auction_state = load_auction(program_id, auction)?;
    if admin.address() != &auction_state.admin {
        return Err(DepositProgramError::InvalidAdmin.into());
    }
    let mut bid_state = load_bid(program_id, auction, bid)?;

    let now = Clock::get()?.unix_timestamp;
    if now <= auction_state.bid_deadline {
        return Err(DepositProgramError::BidDeadlineNotReached.into());
    }
    if auction_state.has_winner() {
        return Err(DepositProgramError::WinnerAlreadyDeclared.into());
    }
    if bid_state.state != BidState::Deposited {
        return Err(DepositProgramError::InvalidBidState.into());
    }
    let remainder_deadline =
        now.checked_add(auction_state.payment_window).ok_or(DepositProgramError::ArithmeticOverflow)?;
    if remainder_deadline >= auction_state.escape_time {
        return Err(DepositProgramError::PaymentWindowPassesEscape.into());
    }

    bid_state.state = BidState::Winner;
    bid_state.remainder_deadline = remainder_deadline;
    auction_state.winner_bid = *bid.address();
    write_account(bid, &bid_state.to_bytes())?;
    write_account(auction, &auction_state.to_bytes())?;

    let event = BidEvent {
        kind: EventKind::WinnerDeclared,
        auction: *auction.address(),
        bid: *bid.address(),
        bidder: bid_state.bidder,
        recipient: Address::new_from_array([0u8; 32]),
        amount: bid_state.amount,
    };
    emit_event(program_id, event_authority, current_program, &event.to_bytes())
}
