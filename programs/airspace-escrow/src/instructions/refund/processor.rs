use pinocchio::{account::AccountView, Address, ProgramResult};

use crate::instructions::{process_admin_payout, Payout, Refund};

/// Processes the Refund instruction.
///
/// The escrow admin sends a receipt's whole amount back to its depositor and closes the receipt.
pub fn process_refund(program_id: &Address, accounts: &[AccountView], instruction_data: &[u8]) -> ProgramResult {
    let ix = Refund::try_from((instruction_data, accounts))?;
    process_admin_payout(program_id, &ix.accounts, Payout::Refund)
}
