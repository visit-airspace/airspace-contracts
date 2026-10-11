use pinocchio::{
    account::AccountView,
    cpi::Seed,
    error::ProgramError,
    sysvars::{clock::Clock, Sysvar},
    Address, ProgramResult,
};
use pinocchio_associated_token_account::instructions::CreateIdempotent;

use crate::{
    errors::DepositProgramError,
    events::AuctionCreatedEvent,
    state::{Auction, AUCTION_SEED},
    utils::{
        associated_token_address, auction_address, create_pda_account, emit_event, exact,
        verify_associated_token_program, verify_mint, verify_new_treasury, verify_readonly, verify_signer,
        verify_system_program, verify_token_program, verify_writable, write_account,
    },
};

/// The fields of CreateAuction after the discriminator.
struct CreateAuctionData {
    auction_id: Address,
    deposit_bps: u16,
    min_deposit: u64,
    bid_deadline: i64,
    payment_window: i64,
    escape_time: i64,
}

impl CreateAuctionData {
    const LEN: usize = 32 + 2 + 8 * 4;

    fn parse(data: &[u8]) -> Result<Self, ProgramError> {
        let data = exact(data, Self::LEN)?;
        let word = |from: usize| -> Result<[u8; 8], ProgramError> {
            data[from..from + 8].try_into().map_err(|_| ProgramError::InvalidInstructionData)
        };
        let id: [u8; 32] = data[0..32].try_into().map_err(|_| ProgramError::InvalidInstructionData)?;
        let bps: [u8; 2] = data[32..34].try_into().map_err(|_| ProgramError::InvalidInstructionData)?;
        Ok(Self {
            auction_id: Address::new_from_array(id),
            deposit_bps: u16::from_le_bytes(bps),
            min_deposit: u64::from_le_bytes(word(34)?),
            bid_deadline: i64::from_le_bytes(word(42)?),
            payment_window: i64::from_le_bytes(word(50)?),
            escape_time: i64::from_le_bytes(word(58)?),
        })
    }
}

/// Processes the CreateAuction instruction.
///
/// # Account Layout
/// 0. `[writable, signer]` admin - Pays for the auction account and becomes the auction admin
/// 1. `[writable]` auction - Auction PDA at seeds `["auction", admin, auction_id]`
/// 2. `[]` mint - The auction's mint, 6 decimals and no token extensions
/// 3. `[]` treasury - Associated token account for the mint, without extensions, that receives every
///    settled sale and forfeit
/// 4. `[writable]` vault - Associated token account of the auction for the mint
/// 5. `[]` token_program
/// 6. `[]` associated_token_program
/// 7. `[]` system_program
/// 8. `[]` event_authority
/// 9. `[]` program
pub fn process_create_auction(
    program_id: &Address,
    accounts: &[AccountView],
    instruction_data: &[u8],
) -> ProgramResult {
    let [admin, auction, mint, treasury, vault, token_program, associated_token_program, system_program, event_authority, current_program] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    let data = CreateAuctionData::parse(instruction_data)?;

    verify_signer(admin, true)?;
    verify_writable(auction, true)?;
    verify_writable(vault, true)?;
    verify_readonly(mint)?;
    verify_readonly(treasury)?;
    verify_token_program(token_program)?;
    verify_associated_token_program(associated_token_program)?;
    verify_system_program(system_program)?;
    verify_mint(mint, token_program)?;
    verify_new_treasury(treasury, mint, token_program)?;

    let (expected_auction, bump) = auction_address(program_id, admin.address(), &data.auction_id);
    if auction.address() != &expected_auction {
        return Err(DepositProgramError::InvalidSeeds.into());
    }
    let expected_vault = associated_token_address(auction.address(), mint.address(), token_program.address());
    if vault.address() != &expected_vault {
        return Err(DepositProgramError::InvalidVault.into());
    }

    let state = Auction {
        bump,
        auction_id: data.auction_id,
        admin: *admin.address(),
        mint: *mint.address(),
        treasury: *treasury.address(),
        vault: expected_vault,
        deposit_bps: data.deposit_bps,
        min_deposit: data.min_deposit,
        bid_deadline: data.bid_deadline,
        payment_window: data.payment_window,
        escape_time: data.escape_time,
        winner_bid: Address::new_from_array([0u8; 32]),
    };
    state.validate_terms(Clock::get()?.unix_timestamp)?;

    let bump_seed = [bump];
    let seeds: [Seed; 4] = [
        Seed::from(AUCTION_SEED),
        Seed::from(admin.address().as_ref()),
        Seed::from(data.auction_id.as_ref()),
        Seed::from(bump_seed.as_slice()),
    ];
    create_pda_account(admin, Auction::LEN, program_id, auction, seeds)?;
    write_account(auction, &state.to_bytes())?;

    CreateIdempotent { funding_account: admin, account: vault, wallet: auction, mint, system_program, token_program }
        .invoke()?;

    let event = AuctionCreatedEvent {
        auction: *auction.address(),
        admin: *admin.address(),
        mint: *mint.address(),
        treasury: *treasury.address(),
        deposit_bps: data.deposit_bps,
        min_deposit: data.min_deposit,
        bid_deadline: data.bid_deadline,
        payment_window: data.payment_window,
        escape_time: data.escape_time,
    };
    emit_event(program_id, event_authority, current_program, &event.to_bytes())
}
