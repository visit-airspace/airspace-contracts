use pinocchio::{
    account::AccountView,
    error::ProgramError,
    sysvars::{clock::Clock, Sysvar},
    Address, ProgramResult,
};

use crate::{
    events::WithdrawEvent,
    instructions::Withdraw,
    state::{
        check_escape_window, get_extensions_from_account, validate_extensions_pda, ArbiterData, Escrow, ExtensionType,
        HookData, HookPoint, Receipt, TimelockData,
    },
    traits::{EventSerialize, ExtensionData},
    utils::{close_pda_account, emit_event, transfer_from_vault},
};

/// Checks the timelock, the settlement escape time, and the arbiter, and returns the hook (if
/// any) with the remaining accounts that are left for it.
fn read_extensions<'a>(
    program_id: &Address,
    ix: &Withdraw<'a>,
    deposited_at: i64,
) -> Result<(Option<HookData>, &'a [AccountView]), ProgramError> {
    // Validate extensions PDA
    validate_extensions_pda(ix.accounts.escrow, ix.accounts.extensions, program_id)?;

    // Get timelock, hook, arbiter, and settlement extensions in single pass
    let exts = get_extensions_from_account(
        ix.accounts.extensions,
        &[ExtensionType::Timelock, ExtensionType::Hook, ExtensionType::Arbiter, ExtensionType::Settlement],
    )?;

    // An escrow with settlement terms lets a depositor out on their own only from the escape time
    check_escape_window(&exts[3], Clock::get()?.unix_timestamp)?;

    // Validate timelock if present
    if let Some(ref timelock_bytes) = exts[0] {
        let timelock = TimelockData::from_bytes(timelock_bytes)?;
        timelock.validate(deposited_at)?;
    }

    // Parse hook if present
    let hook_data = exts[1].as_ref().map(|b| HookData::from_bytes(b)).transpose()?;

    // Validate arbiter if present, the arbiter is first in remaining_accounts
    let remaining_accounts = if let Some(ref arbiter_bytes) = exts[2] {
        let arbiter = ArbiterData::from_bytes(arbiter_bytes)?;
        arbiter.validate(ix.accounts.remaining_accounts)?;
        // Skip arbiter account, pass rest to hook
        ix.accounts.remaining_accounts.get(1..).unwrap_or(&[])
    } else {
        ix.accounts.remaining_accounts
    };
    Ok((hook_data, remaining_accounts))
}

/// Processes the Withdraw instruction.
///
/// Transfers tokens from escrow vault back to the withdrawer and closes the receipt PDA.
pub fn process_withdraw(program_id: &Address, accounts: &[AccountView], instruction_data: &[u8]) -> ProgramResult {
    let ix = Withdraw::try_from((instruction_data, accounts))?;

    // Validate escrow PDA
    {
        let escrow_data = ix.accounts.escrow.try_borrow()?;
        let _escrow = Escrow::from_account(&escrow_data, ix.accounts.escrow, program_id)?;
    }

    // Read and validate receipt
    let (amount, receipt_seed, receipt_mint, deposited_at) = {
        let receipt_data = ix.accounts.receipt.try_borrow()?;
        let receipt = Receipt::from_account(&receipt_data, ix.accounts.receipt, program_id)?;

        // Verify withdrawer matches the original depositor and the receipt is for this escrow
        receipt.validate_depositor(ix.accounts.escrow.address(), ix.accounts.withdrawer.address())?;

        (receipt.amount, receipt.receipt_seed, receipt.mint, receipt.deposited_at)
    };

    // Ensure the mint account matches the receipt's mint to prevent cross-mint withdrawals.
    if receipt_mint != *ix.accounts.mint.address() {
        return Err(ProgramError::InvalidAccountData);
    }

    let (hook_data, remaining_accounts) = read_extensions(program_id, &ix, deposited_at)?;

    // Invoke pre-withdraw hook if configured
    if let Some(ref hook) = hook_data {
        hook.invoke(
            HookPoint::PreWithdraw,
            remaining_accounts,
            &[ix.accounts.escrow, ix.accounts.withdrawer, ix.accounts.mint, ix.accounts.receipt],
        )?;
    }

    // Transfer tokens from vault to withdrawer using escrow PDA as signer
    transfer_from_vault(
        ix.accounts.escrow,
        ix.accounts.vault,
        ix.accounts.mint,
        ix.accounts.withdrawer_token_account,
        ix.accounts.token_program,
        amount,
    )?;

    // Invoke post-withdraw hook if configured (receipt is still open, pass it for context)
    if let Some(ref hook) = hook_data {
        hook.invoke(
            HookPoint::PostWithdraw,
            remaining_accounts,
            &[ix.accounts.escrow, ix.accounts.withdrawer, ix.accounts.mint, ix.accounts.receipt],
        )?;
    }

    // Close receipt account and return lamports to rent_recipient
    close_pda_account(ix.accounts.receipt, ix.accounts.rent_recipient)?;

    // Emit event
    let event = WithdrawEvent::new(
        *ix.accounts.escrow.address(),
        *ix.accounts.withdrawer.address(),
        receipt_mint,
        receipt_seed,
        amount,
    );
    emit_event(program_id, ix.accounts.event_authority, ix.accounts.escrow_program, &event.to_bytes())?;

    Ok(())
}
