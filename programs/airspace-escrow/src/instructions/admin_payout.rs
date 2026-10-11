//! The part that Refund and Settle share.
//!
//! Both instructions are signed by the escrow admin, pay one receipt's whole amount to
//! one recipient, close the receipt, and return its rent to the depositor who paid it.
//! They differ only in who the recipient is: the receipt's depositor for Refund, and
//! the beneficiary of the settlement terms for Settle.

use pinocchio::{account::AccountView, error::ProgramError, Address, ProgramResult};

use crate::{
    errors::EscrowProgramError,
    events::{RefundEvent, SettleEvent},
    state::{
        get_extensions_from_account, validate_extensions_pda, Escrow, ExtensionType, HookData, HookPoint, Receipt,
        SettlementData,
    },
    traits::{EventSerialize, ExtensionData, InstructionAccounts},
    utils::{
        close_pda_account, emit_event, transfer_from_vault, validate_associated_token_account, verify_current_program,
        verify_current_program_account, verify_event_authority, verify_owned_by, verify_readonly, verify_signer,
        verify_system_program, verify_token_program, verify_writable,
    },
};

/// Who receives the money of a receipt.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Payout {
    /// The receipt's depositor gets their own deposit back.
    Refund,
    /// The beneficiary of the settlement terms gets the deposit.
    Settle,
}

/// Accounts for the Refund and Settle instructions
///
/// # Account Layout
/// 0. `[writable]` rent_recipient - Must be the receipt's depositor, who paid its rent
/// 1. `[signer]` admin - Must match escrow.admin
/// 2. `[]` escrow - Escrow PDA (signing authority for the vault transfer)
/// 3. `[]` extensions - Extensions PDA holding the settlement terms
/// 4. `[writable]` receipt - Deposit receipt to pay out and close
/// 5. `[writable]` vault - Escrow's vault token account (source)
/// 6. `[writable]` recipient_token_account - Associated token account of the depositor (Refund) or of the beneficiary (Settle)
/// 7. `[]` mint - Token mint
/// 8. `[]` token_program - SPL Token program
/// 9. `[]` system_program - System program
/// 10. `[]` event_authority - Event authority PDA
/// 11. `[]` escrow_program - Current program
///
/// # Remaining Accounts
/// If a hook extension is set:
///   0. `[]` hook_program - The hook program to invoke
///   1..N. `[]` extra accounts - Additional accounts to pass to the hook (all read-only)
pub struct AdminPayoutAccounts<'a> {
    pub rent_recipient: &'a AccountView,
    pub admin: &'a AccountView,
    pub escrow: &'a AccountView,
    pub extensions: &'a AccountView,
    pub receipt: &'a AccountView,
    pub vault: &'a AccountView,
    pub recipient_token_account: &'a AccountView,
    pub mint: &'a AccountView,
    pub token_program: &'a AccountView,
    pub system_program: &'a AccountView,
    pub event_authority: &'a AccountView,
    pub escrow_program: &'a AccountView,
    pub remaining_accounts: &'a [AccountView],
}

impl<'a> TryFrom<&'a [AccountView]> for AdminPayoutAccounts<'a> {
    type Error = ProgramError;

    #[inline(always)]
    fn try_from(accounts: &'a [AccountView]) -> Result<Self, Self::Error> {
        let [rent_recipient, admin, escrow, extensions, receipt, vault, recipient_token_account, mint, token_program, system_program, event_authority, escrow_program, remaining_accounts @ ..] =
            accounts
        else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };

        // 1. Validate signers
        verify_signer(admin, false)?;

        // 2. Validate writable
        verify_writable(rent_recipient, true)?;
        verify_writable(receipt, true)?;
        verify_writable(vault, true)?;
        verify_writable(recipient_token_account, true)?;

        // 3. Validate readonly
        verify_readonly(escrow)?;
        verify_readonly(extensions)?;
        verify_readonly(mint)?;

        // 4. Validate program IDs
        verify_token_program(token_program)?;
        verify_owned_by(mint, token_program.address())?;
        verify_system_program(system_program)?;
        verify_current_program(escrow_program)?;
        verify_event_authority(event_authority)?;

        // 5. Validate accounts owned by current program
        verify_current_program_account(escrow)?;
        verify_current_program_account(receipt)?;

        // 6. Validate the vault, which only the escrow can own
        validate_associated_token_account(vault, escrow.address(), mint, token_program)?;

        Ok(Self {
            rent_recipient,
            admin,
            escrow,
            extensions,
            receipt,
            vault,
            recipient_token_account,
            mint,
            token_program,
            system_program,
            event_authority,
            escrow_program,
            remaining_accounts,
        })
    }
}

impl<'a> InstructionAccounts<'a> for AdminPayoutAccounts<'a> {}

/// Pays out one receipt as `payout` says, closes it, and emits the event.
pub fn process_admin_payout(program_id: &Address, accounts: &AdminPayoutAccounts, payout: Payout) -> ProgramResult {
    // Validate the escrow and that the signer is its admin
    {
        let escrow_data = accounts.escrow.try_borrow()?;
        let escrow = Escrow::from_account(&escrow_data, accounts.escrow, program_id)?;
        escrow.validate_admin(accounts.admin.address())?;
    }

    let (amount, depositor) = read_receipt(program_id, accounts)?;

    // The settlement terms decide who the beneficiary is, and only an escrow that has them pays out
    validate_extensions_pda(accounts.escrow, accounts.extensions, program_id)?;
    let exts = get_extensions_from_account(accounts.extensions, &[ExtensionType::Settlement, ExtensionType::Hook])?;
    let settlement_bytes = exts[0].as_ref().ok_or(EscrowProgramError::SettlementNotSet)?;
    let settlement = SettlementData::from_bytes(settlement_bytes)?;
    let hook_data = exts[1].as_ref().map(|bytes| HookData::from_bytes(bytes)).transpose()?;

    // The money can reach exactly one address: the associated token account of the recipient
    let recipient = match payout {
        Payout::Refund => depositor,
        Payout::Settle => settlement.beneficiary,
    };
    validate_associated_token_account(
        accounts.recipient_token_account,
        &recipient,
        accounts.mint,
        accounts.token_program,
    )
    .map_err(|_| ProgramError::from(EscrowProgramError::InvalidPayoutRecipient))?;

    // Invoke the pre-withdraw hook if configured
    if let Some(ref hook) = hook_data {
        hook.invoke(
            HookPoint::PreWithdraw,
            accounts.remaining_accounts,
            &[accounts.escrow, accounts.admin, accounts.mint, accounts.receipt],
        )?;
    }

    // Move the whole amount from the vault, with the escrow PDA as signer
    transfer_from_vault(
        accounts.escrow,
        accounts.vault,
        accounts.mint,
        accounts.recipient_token_account,
        accounts.token_program,
        amount,
    )?;

    // Invoke the post-withdraw hook if configured, with the receipt still open
    if let Some(ref hook) = hook_data {
        hook.invoke(
            HookPoint::PostWithdraw,
            accounts.remaining_accounts,
            &[accounts.escrow, accounts.admin, accounts.mint, accounts.receipt],
        )?;
    }

    // Close the receipt and return its rent to the depositor
    close_pda_account(accounts.receipt, accounts.rent_recipient)?;

    emit_payout_event(program_id, accounts, payout, depositor, amount)
}

/// Reads the receipt to pay out, which must belong to this escrow and to the mint account, and
/// whose rent must go back to the depositor who paid it, whoever signs.
fn read_receipt(program_id: &Address, accounts: &AdminPayoutAccounts) -> Result<(u64, Address), ProgramError> {
    let receipt_data = accounts.receipt.try_borrow()?;
    let receipt = Receipt::from_account(&receipt_data, accounts.receipt, program_id)?;
    if receipt.escrow != *accounts.escrow.address() {
        return Err(EscrowProgramError::InvalidReceiptEscrow.into());
    }
    // One mint's receipt never pays another mint
    if receipt.mint != *accounts.mint.address() {
        return Err(ProgramError::InvalidAccountData);
    }
    if accounts.rent_recipient.address() != &receipt.depositor {
        return Err(EscrowProgramError::InvalidRentRecipient.into());
    }
    Ok((receipt.amount, receipt.depositor))
}

/// Emits the Refund or Settle event for the receipt that was just paid out.
fn emit_payout_event(
    program_id: &Address,
    accounts: &AdminPayoutAccounts,
    payout: Payout,
    depositor: Address,
    amount: u64,
) -> ProgramResult {
    let escrow = *accounts.escrow.address();
    let receipt = *accounts.receipt.address();
    let recipient = *accounts.recipient_token_account.address();
    let mint = *accounts.mint.address();
    let event_bytes = match payout {
        Payout::Refund => RefundEvent::new(escrow, receipt, depositor, recipient, mint, amount).to_bytes(),
        Payout::Settle => SettleEvent::new(escrow, receipt, depositor, recipient, mint, amount).to_bytes(),
    };
    emit_event(program_id, accounts.event_authority, accounts.escrow_program, &event_bytes)
}
