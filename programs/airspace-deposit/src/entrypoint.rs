use pinocchio::{account::AccountView, entrypoint, error::ProgramError, Address, ProgramResult};

use crate::instructions::{
    process_create_auction, process_declare_winner, process_emit_event, process_escape, process_forfeit,
    process_pay_remainder, process_place_deposit, process_refund_deposit, process_settle, DepositInstruction,
};

entrypoint!(process_instruction);

pub fn process_instruction(program_id: &Address, accounts: &[AccountView], instruction_data: &[u8]) -> ProgramResult {
    let (discriminator, instruction_data) =
        instruction_data.split_first().ok_or(ProgramError::InvalidInstructionData)?;

    match DepositInstruction::try_from(*discriminator)? {
        DepositInstruction::CreateAuction => process_create_auction(program_id, accounts, instruction_data),
        DepositInstruction::PlaceDeposit => process_place_deposit(program_id, accounts, instruction_data),
        DepositInstruction::RefundDeposit => process_refund_deposit(program_id, accounts, instruction_data),
        DepositInstruction::DeclareWinner => process_declare_winner(program_id, accounts, instruction_data),
        DepositInstruction::PayRemainder => process_pay_remainder(program_id, accounts, instruction_data),
        DepositInstruction::Settle => process_settle(program_id, accounts, instruction_data),
        DepositInstruction::Forfeit => process_forfeit(program_id, accounts, instruction_data),
        DepositInstruction::Escape => process_escape(program_id, accounts, instruction_data),
        DepositInstruction::EmitEvent => process_emit_event(program_id, accounts),
    }
}
