use alloc::vec::Vec;
use pinocchio::{account::AccountView, cpi::Seed, error::ProgramError, Address, ProgramResult};

use crate::{
    errors::EscrowProgramError,
    events::SettlementSetEvent,
    instructions::SetSettlement,
    state::{
        get_extensions_from_account, update_or_append_extension, Escrow, ExtensionType, ExtensionsPda, SettlementData,
    },
    traits::{EventSerialize, ExtensionData, PdaSeeds},
    utils::emit_event,
};

/// Processes the SetSettlement instruction.
///
/// Gives an escrow its settlement terms: the beneficiary, the deposit deadline, and the
/// escape time. The terms are set once and never change, and RemoveExtension refuses to
/// remove them, so a depositor who reads them on chain knows the rules before depositing.
pub fn process_set_settlement(
    program_id: &Address,
    accounts: &[AccountView],
    instruction_data: &[u8],
) -> ProgramResult {
    let ix = SetSettlement::try_from((instruction_data, accounts))?;

    // Read escrow and validate
    let escrow_data = ix.accounts.escrow.try_borrow()?;
    let escrow = Escrow::from_account(&escrow_data, ix.accounts.escrow, program_id)?;
    escrow.validate_admin(ix.accounts.admin.address())?;
    escrow.require_mutable()?;

    // Validate extensions PDA
    let extensions_pda = ExtensionsPda::new(ix.accounts.escrow.address());
    extensions_pda.validate_pda(ix.accounts.extensions, program_id, ix.data.extensions_bump)?;

    // The terms are usable, and this is the first time they are set
    let settlement = SettlementData::new(ix.data.beneficiary, ix.data.deposit_deadline, ix.data.escape_time);
    settlement.validate_terms()?;
    let existing = get_extensions_from_account(ix.accounts.extensions, &[ExtensionType::Settlement])?;
    if existing[0].is_some() {
        return Err(EscrowProgramError::SettlementAlreadySet.into());
    }

    // Get seeds and append the extension
    let extensions_bump_seed = [ix.data.extensions_bump];
    let extensions_seeds: Vec<Seed> = extensions_pda.seeds_with_bump(&extensions_bump_seed);
    let extensions_seeds_array: [Seed; 3] = extensions_seeds.try_into().map_err(|_| ProgramError::InvalidArgument)?;

    update_or_append_extension(
        ix.accounts.payer,
        ix.accounts.extensions,
        program_id,
        ix.data.extensions_bump,
        ExtensionType::Settlement,
        &settlement.to_bytes(),
        extensions_seeds_array,
    )?;

    // Emit event
    let event = SettlementSetEvent::new(
        *ix.accounts.escrow.address(),
        ix.data.beneficiary,
        ix.data.deposit_deadline,
        ix.data.escape_time,
    );
    emit_event(program_id, ix.accounts.event_authority, ix.accounts.escrow_program, &event.to_bytes())?;

    Ok(())
}
