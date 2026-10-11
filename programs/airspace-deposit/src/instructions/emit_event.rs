use pinocchio::{account::AccountView, error::ProgramError, Address, ProgramResult};

use crate::utils::{verify_event_authority, verify_signer};

/// Processes the EmitEvent instruction.
///
/// A no-op that only checks that the event authority PDA signed. The event is the instruction data.
pub fn process_emit_event(_program_id: &Address, accounts: &[AccountView]) -> ProgramResult {
    let [event_authority] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    verify_event_authority(event_authority)?;
    verify_signer(event_authority, false)
}
