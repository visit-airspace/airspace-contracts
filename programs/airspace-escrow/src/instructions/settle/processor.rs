use pinocchio::{account::AccountView, Address, ProgramResult};

use crate::instructions::{process_admin_payout, Payout, Settle};

/// Processes the Settle instruction.
///
/// The escrow admin sends a receipt's whole amount to the beneficiary of the settlement
/// terms and closes the receipt.
pub fn process_settle(program_id: &Address, accounts: &[AccountView], instruction_data: &[u8]) -> ProgramResult {
    let ix = Settle::try_from((instruction_data, accounts))?;
    process_admin_payout(program_id, &ix.accounts, Payout::Settle)
}
