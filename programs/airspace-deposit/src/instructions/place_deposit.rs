use pinocchio::{
    account::AccountView,
    cpi::Seed,
    error::ProgramError,
    sysvars::{clock::Clock, Sysvar},
    Address, ProgramResult,
};

use crate::{
    errors::DepositProgramError,
    events::{BidEvent, EventKind},
    state::{Bid, BidState, BID_SEED},
    utils::{
        bid_address, create_pda_account, emit_event, exact, load_auction, transfer_from_wallet, verify_readonly,
        verify_signer, verify_system_program, verify_token_account, verify_vault, verify_writable, write_account,
    },
};

/// Processes the PlaceDeposit instruction.
///
/// Moves the deposit that the program computes from the bid amount into the vault and creates the
/// bid in state `deposited`. The bidder pays the rent of the bid.
///
/// # Account Layout
/// 0. `[writable, signer]` bidder
/// 1. `[]` auction
/// 2. `[writable]` bid - Bid PDA at seeds `["bid", auction, bidder, bid_seed]`
/// 3. `[writable]` vault - The auction's vault
/// 4. `[writable]` bidder_token_account - The bidder's token account for the mint, the source
/// 5. `[]` mint
/// 6. `[]` token_program
/// 7. `[]` system_program
/// 8. `[]` event_authority
/// 9. `[]` program
///
/// # Data
/// bid_seed (32 bytes), amount u64
pub fn process_place_deposit(program_id: &Address, accounts: &[AccountView], instruction_data: &[u8]) -> ProgramResult {
    let [bidder, auction, bid, vault, bidder_token_account, mint, token_program, system_program, event_authority, current_program] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    let data = exact(instruction_data, 32 + 8)?;
    let bid_seed = Address::new_from_array(data[0..32].try_into().map_err(|_| ProgramError::InvalidInstructionData)?);
    let amount = u64::from_le_bytes(data[32..40].try_into().map_err(|_| ProgramError::InvalidInstructionData)?);

    verify_signer(bidder, true)?;
    verify_readonly(auction)?;
    verify_writable(bid, true)?;
    verify_writable(vault, true)?;
    verify_writable(bidder_token_account, true)?;
    verify_readonly(mint)?;
    verify_system_program(system_program)?;

    let state = load_auction(program_id, auction)?;
    verify_vault(&state, vault, mint, token_program)?;
    verify_token_account(bidder_token_account, mint.address(), token_program)?;

    let now = Clock::get()?.unix_timestamp;
    if now > state.bid_deadline {
        return Err(DepositProgramError::BidDeadlinePassed.into());
    }
    let deposit = state.deposit_for(amount)?;

    let (expected_bid, bump) = bid_address(program_id, auction.address(), bidder.address(), &bid_seed);
    if bid.address() != &expected_bid {
        return Err(DepositProgramError::InvalidSeeds.into());
    }
    let bid_state = Bid {
        bump,
        state: BidState::Deposited,
        auction: *auction.address(),
        bidder: *bidder.address(),
        bid_seed,
        amount,
        deposit,
        remainder_paid: 0,
        remainder_deadline: 0,
    };

    let bump_seed = [bump];
    let seeds: [Seed; 5] = [
        Seed::from(BID_SEED),
        Seed::from(auction.address().as_ref()),
        Seed::from(bidder.address().as_ref()),
        Seed::from(bid_seed.as_ref()),
        Seed::from(bump_seed.as_slice()),
    ];
    create_pda_account(bidder, Bid::LEN, program_id, bid, seeds)?;
    write_account(bid, &bid_state.to_bytes())?;

    transfer_from_wallet(bidder_token_account, vault, mint, bidder, token_program, deposit)?;

    let event = BidEvent {
        kind: EventKind::DepositPlaced,
        auction: *auction.address(),
        bid: *bid.address(),
        bidder: *bidder.address(),
        recipient: state.vault,
        amount: deposit,
    };
    emit_event(program_id, event_authority, current_program, &event.to_bytes())
}
