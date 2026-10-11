use solana_sdk::pubkey::Pubkey;

use crate::utils::ESCROW_PROGRAM_ID;

pub fn find_escrow_pda(escrow_seed: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"escrow", escrow_seed.as_ref()], &ESCROW_PROGRAM_ID)
}

pub fn find_extensions_pda(escrow: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"extensions", escrow.as_ref()], &ESCROW_PROGRAM_ID)
}

pub fn find_event_authority_pda() -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"event_authority"], &ESCROW_PROGRAM_ID)
}

pub fn find_receipt_pda(escrow: &Pubkey, depositor: &Pubkey, mint: &Pubkey, receipt_seed: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[b"receipt", escrow.as_ref(), depositor.as_ref(), mint.as_ref(), receipt_seed.as_ref()],
        &ESCROW_PROGRAM_ID,
    )
}

pub fn find_allowed_mint_pda(escrow: &Pubkey, mint: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"allowed_mint", escrow.as_ref(), mint.as_ref()], &ESCROW_PROGRAM_ID)
}
