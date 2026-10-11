//! Tests for the AirSpace settlement terms: SetSettlement, the deposit deadline,
//! the escape time on Withdraw, and the admin payouts Refund and Settle.

use solana_sdk::{
    instruction::InstructionError,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
};

use crate::{
    client::{
        AddTimelockBuilder, AllowMintBuilder, CreatesEscrowBuilder, DepositBuilder, RefundBuilder,
        RemoveExtensionBuilder, SetSettlementBuilder, SettleBuilder, WithdrawBuilder,
    },
    utils::{
        assert_account_not_exists, assert_escrow_error, assert_extensions_header, assert_instruction_error,
        find_allowed_mint_pda, find_escrow_pda, find_extensions_pda, find_receipt_pda, EscrowError, TestContext,
    },
};

const START: i64 = 1_000_000;
const DAY: i64 = 86_400;
const DEPOSIT_DEADLINE: i64 = START + 10 * DAY;
const ESCAPE_TIME: i64 = DEPOSIT_DEADLINE + 7 * DAY;
const AMOUNT: u64 = 5_000_000;
const SETTLEMENT_TYPE: u16 = 4;

struct Auction {
    admin: Keypair,
    escrow: Pubkey,
    extensions: Pubkey,
    extensions_bump: u8,
    mint: Keypair,
    vault: Pubkey,
    allowed_mint: Pubkey,
    allowed_mint_bump: u8,
    beneficiary: Pubkey,
    beneficiary_ata: Pubkey,
}

struct Deposit {
    depositor: Keypair,
    depositor_ata: Pubkey,
    receipt: Pubkey,
}

impl Auction {
    /// An escrow whose settlement terms are set, with the mint not yet allowed.
    fn new(ctx: &mut TestContext, with_terms: bool) -> Self {
        ctx.warp_to_timestamp(START);
        let admin = ctx.create_funded_keypair();
        let seed = Keypair::new();
        let (escrow, bump) = find_escrow_pda(&seed.pubkey());
        let ix = CreatesEscrowBuilder::new()
            .payer(ctx.payer.pubkey())
            .admin(admin.pubkey())
            .escrow_seed(seed.pubkey())
            .escrow(escrow)
            .bump(bump)
            .instruction();
        ctx.send_transaction(ix, &[&admin, &seed]).unwrap();

        let (extensions, extensions_bump) = find_extensions_pda(&escrow);
        let mint = Keypair::new();
        ctx.create_mint(&mint, &ctx.payer.pubkey(), 6);
        let vault = ctx.create_token_account(&escrow, &mint.pubkey());
        let (allowed_mint, allowed_mint_bump) = find_allowed_mint_pda(&escrow, &mint.pubkey());
        let beneficiary = Pubkey::new_unique();
        let beneficiary_ata = ctx.create_token_account(&beneficiary, &mint.pubkey());

        let auction = Self {
            admin,
            escrow,
            extensions,
            extensions_bump,
            mint,
            vault,
            allowed_mint,
            allowed_mint_bump,
            beneficiary,
            beneficiary_ata,
        };
        if with_terms {
            auction.set_terms(ctx, DEPOSIT_DEADLINE, ESCAPE_TIME);
        }
        auction
    }

    fn terms_instruction(
        &self,
        ctx: &TestContext,
        signer: &Keypair,
        deposit_deadline: i64,
        escape_time: i64,
    ) -> solana_sdk::instruction::Instruction {
        SetSettlementBuilder::new()
            .payer(ctx.payer.pubkey())
            .admin(signer.pubkey())
            .escrow(self.escrow)
            .extensions(self.extensions)
            .extensions_bump(self.extensions_bump)
            .beneficiary(self.beneficiary)
            .deposit_deadline(deposit_deadline)
            .escape_time(escape_time)
            .instruction()
    }

    fn set_terms(&self, ctx: &mut TestContext, deposit_deadline: i64, escape_time: i64) {
        let ix = self.terms_instruction(ctx, &self.admin, deposit_deadline, escape_time);
        ctx.send_transaction(ix, &[&self.admin]).unwrap();
    }

    fn set_terms_error(
        &self,
        ctx: &mut TestContext,
        signer: &Keypair,
        deposit_deadline: i64,
        escape_time: i64,
    ) -> solana_sdk::transaction::TransactionError {
        ctx.expire_blockhash();
        let ix = self.terms_instruction(ctx, signer, deposit_deadline, escape_time);
        ctx.send_transaction_expect_error(ix, &[signer])
    }

    fn allow_mint(&self, ctx: &mut TestContext) {
        let ix = AllowMintBuilder::new()
            .payer(ctx.payer.pubkey())
            .admin(self.admin.pubkey())
            .escrow(self.escrow)
            .escrow_extensions(self.extensions)
            .mint(self.mint.pubkey())
            .allowed_mint(self.allowed_mint)
            .vault(self.vault)
            .bump(self.allowed_mint_bump)
            .instruction();
        ctx.send_transaction(ix, &[&self.admin]).unwrap();
    }

    fn deposit_instruction(&self, ctx: &mut TestContext) -> (solana_sdk::instruction::Instruction, Deposit, Keypair) {
        let depositor = ctx.create_funded_keypair();
        let depositor_ata = ctx.create_token_account_with_balance(&depositor.pubkey(), &self.mint.pubkey(), AMOUNT * 2);
        let seed = Keypair::new();
        let (receipt, bump) = find_receipt_pda(&self.escrow, &depositor.pubkey(), &self.mint.pubkey(), &seed.pubkey());
        let ix = DepositBuilder::new()
            .payer(ctx.payer.pubkey())
            .depositor(depositor.pubkey())
            .escrow(self.escrow)
            .allowed_mint(self.allowed_mint)
            .receipt_seed(seed.pubkey())
            .receipt(receipt)
            .vault(self.vault)
            .depositor_token_account(depositor_ata)
            .mint(self.mint.pubkey())
            .extensions(self.extensions)
            .bump(bump)
            .amount(AMOUNT)
            .instruction();
        (ix, Deposit { depositor, depositor_ata, receipt }, seed)
    }

    fn deposit(&self, ctx: &mut TestContext) -> Deposit {
        let (ix, deposit, seed) = self.deposit_instruction(ctx);
        ctx.send_transaction(ix, &[&deposit.depositor, &seed]).unwrap();
        deposit
    }

    fn payout_accounts(&self, deposit: &Deposit, recipient_ata: Pubkey) -> PayoutAccounts {
        PayoutAccounts {
            rent_recipient: deposit.depositor.pubkey(),
            admin: self.admin.pubkey(),
            escrow: self.escrow,
            extensions: self.extensions,
            receipt: deposit.receipt,
            vault: self.vault,
            recipient_ata,
            mint: self.mint.pubkey(),
        }
    }

    fn withdraw_instruction(&self, deposit: &Deposit) -> solana_sdk::instruction::Instruction {
        WithdrawBuilder::new()
            .rent_recipient(deposit.depositor.pubkey())
            .withdrawer(deposit.depositor.pubkey())
            .escrow(self.escrow)
            .extensions(self.extensions)
            .receipt(deposit.receipt)
            .vault(self.vault)
            .withdrawer_token_account(deposit.depositor_ata)
            .mint(self.mint.pubkey())
            .instruction()
    }
}

struct PayoutAccounts {
    rent_recipient: Pubkey,
    admin: Pubkey,
    escrow: Pubkey,
    extensions: Pubkey,
    receipt: Pubkey,
    vault: Pubkey,
    recipient_ata: Pubkey,
    mint: Pubkey,
}

fn refund_ix(a: &PayoutAccounts) -> solana_sdk::instruction::Instruction {
    RefundBuilder::new()
        .rent_recipient(a.rent_recipient)
        .admin(a.admin)
        .escrow(a.escrow)
        .extensions(a.extensions)
        .receipt(a.receipt)
        .vault(a.vault)
        .recipient_token_account(a.recipient_ata)
        .mint(a.mint)
        .instruction()
}

fn settle_ix(a: &PayoutAccounts) -> solana_sdk::instruction::Instruction {
    SettleBuilder::new()
        .rent_recipient(a.rent_recipient)
        .admin(a.admin)
        .escrow(a.escrow)
        .extensions(a.extensions)
        .receipt(a.receipt)
        .vault(a.vault)
        .recipient_token_account(a.recipient_ata)
        .mint(a.mint)
        .instruction()
}

// ============================================================================
// SetSettlement
// ============================================================================

#[test]
fn test_set_settlement_stores_the_terms() {
    let mut ctx = TestContext::new();
    let auction = Auction::new(&mut ctx, true);
    let account = ctx.get_account(&auction.extensions).expect("extensions account");
    assert_extensions_header(&ctx, &auction.extensions, auction.extensions_bump, 1);
    let tlv = crate::utils::extensions_utils::find_extension(&account.data, SETTLEMENT_TYPE).expect("settlement");
    assert_eq!(tlv.len(), 48);
    assert_eq!(&tlv[0..32], auction.beneficiary.as_ref());
    assert_eq!(i64::from_le_bytes(tlv[32..40].try_into().unwrap()), DEPOSIT_DEADLINE);
    assert_eq!(i64::from_le_bytes(tlv[40..48].try_into().unwrap()), ESCAPE_TIME);
}

#[test]
fn test_set_settlement_twice_is_refused() {
    let mut ctx = TestContext::new();
    let auction = Auction::new(&mut ctx, true);
    let error = auction.set_terms_error(&mut ctx, &auction.admin, DEPOSIT_DEADLINE + DAY, ESCAPE_TIME + DAY);
    assert_escrow_error(error, EscrowError::SettlementAlreadySet);
}

#[test]
fn test_set_settlement_escape_must_be_seven_days_after_deadline() {
    let mut ctx = TestContext::new();
    let auction = Auction::new(&mut ctx, false);
    let error = auction.set_terms_error(&mut ctx, &auction.admin, DEPOSIT_DEADLINE, DEPOSIT_DEADLINE + 7 * DAY - 1);
    assert_escrow_error(error, EscrowError::InvalidSettlementTerms);
    auction.set_terms(&mut ctx, DEPOSIT_DEADLINE, DEPOSIT_DEADLINE + 7 * DAY);
}

#[test]
fn test_set_settlement_requires_the_admin() {
    let mut ctx = TestContext::new();
    let auction = Auction::new(&mut ctx, false);
    let stranger = ctx.create_funded_keypair();
    let error = auction.set_terms_error(&mut ctx, &stranger, DEPOSIT_DEADLINE, ESCAPE_TIME);
    assert_escrow_error(error, EscrowError::InvalidAdmin);
}

#[test]
fn test_set_settlement_cannot_be_removed() {
    let mut ctx = TestContext::new();
    let auction = Auction::new(&mut ctx, true);
    let ix = RemoveExtensionBuilder::new()
        .payer(ctx.payer.pubkey())
        .admin(auction.admin.pubkey())
        .escrow(auction.escrow)
        .extensions(auction.extensions)
        .extensions_bump(auction.extensions_bump)
        .extension_type(SETTLEMENT_TYPE)
        .instruction();
    let error = ctx.send_transaction_expect_error(ix, &[&auction.admin]);
    assert_escrow_error(error, EscrowError::SettlementTermsPermanent);
}

#[test]
fn test_set_settlement_beside_a_timelock_keeps_both() {
    let mut ctx = TestContext::new();
    let auction = Auction::new(&mut ctx, false);
    let ix = AddTimelockBuilder::new()
        .payer(ctx.payer.pubkey())
        .admin(auction.admin.pubkey())
        .escrow(auction.escrow)
        .extensions(auction.extensions)
        .extensions_bump(auction.extensions_bump)
        .lock_duration(60)
        .instruction();
    ctx.send_transaction(ix, &[&auction.admin]).unwrap();
    auction.set_terms(&mut ctx, DEPOSIT_DEADLINE, ESCAPE_TIME);
    assert_extensions_header(&ctx, &auction.extensions, auction.extensions_bump, 2);
}

// ============================================================================
// Deposit deadline and escape time
// ============================================================================

#[test]
fn test_deposit_allowed_until_the_deadline() {
    let mut ctx = TestContext::new();
    let auction = Auction::new(&mut ctx, true);
    auction.allow_mint(&mut ctx);
    ctx.warp_to_timestamp(DEPOSIT_DEADLINE);
    let deposit = auction.deposit(&mut ctx);
    assert_eq!(ctx.get_token_balance(&auction.vault), AMOUNT);
    assert!(ctx.get_account(&deposit.receipt).is_some());
}

#[test]
fn test_deposit_refused_after_the_deadline() {
    let mut ctx = TestContext::new();
    let auction = Auction::new(&mut ctx, true);
    auction.allow_mint(&mut ctx);
    ctx.warp_to_timestamp(DEPOSIT_DEADLINE + 1);
    let (ix, deposit, seed) = auction.deposit_instruction(&mut ctx);
    let error = ctx.send_transaction_expect_error(ix, &[&deposit.depositor, &seed]);
    assert_escrow_error(error, EscrowError::DepositDeadlinePassed);
}

#[test]
fn test_deposit_without_settlement_has_no_deadline() {
    let mut ctx = TestContext::new();
    let auction = Auction::new(&mut ctx, false);
    auction.allow_mint(&mut ctx);
    ctx.warp_to_timestamp(DEPOSIT_DEADLINE * 100);
    auction.deposit(&mut ctx);
}

#[test]
fn test_withdraw_refused_before_the_escape_time() {
    let mut ctx = TestContext::new();
    let auction = Auction::new(&mut ctx, true);
    auction.allow_mint(&mut ctx);
    let deposit = auction.deposit(&mut ctx);
    ctx.warp_to_timestamp(ESCAPE_TIME - 1);
    let ix = auction.withdraw_instruction(&deposit);
    let error = ctx.send_transaction_expect_error(ix, &[&deposit.depositor]);
    assert_escrow_error(error, EscrowError::EscapeTimeNotReached);
    assert_eq!(ctx.get_token_balance(&auction.vault), AMOUNT);
}

#[test]
fn test_withdraw_allowed_at_the_escape_time() {
    let mut ctx = TestContext::new();
    let auction = Auction::new(&mut ctx, true);
    auction.allow_mint(&mut ctx);
    let deposit = auction.deposit(&mut ctx);
    ctx.warp_to_timestamp(ESCAPE_TIME);
    let ix = auction.withdraw_instruction(&deposit);
    ctx.send_transaction(ix, &[&deposit.depositor]).unwrap();
    assert_eq!(ctx.get_token_balance(&auction.vault), 0);
    assert_eq!(ctx.get_token_balance(&deposit.depositor_ata), AMOUNT * 2);
    assert_account_not_exists(&ctx, &deposit.receipt);
}

// ============================================================================
// Refund
// ============================================================================

#[test]
fn test_refund_pays_the_depositor_and_closes_the_receipt() {
    let mut ctx = TestContext::new();
    let auction = Auction::new(&mut ctx, true);
    auction.allow_mint(&mut ctx);
    let deposit = auction.deposit(&mut ctx);
    assert_eq!(ctx.get_token_balance(&deposit.depositor_ata), AMOUNT);
    let accounts = auction.payout_accounts(&deposit, deposit.depositor_ata);
    ctx.send_transaction(refund_ix(&accounts), &[&auction.admin]).unwrap();
    assert_eq!(ctx.get_token_balance(&deposit.depositor_ata), AMOUNT * 2);
    assert_eq!(ctx.get_token_balance(&auction.vault), 0);
    assert_account_not_exists(&ctx, &deposit.receipt);
}

#[test]
fn test_refund_works_after_the_deadline_and_before_the_escape_time() {
    let mut ctx = TestContext::new();
    let auction = Auction::new(&mut ctx, true);
    auction.allow_mint(&mut ctx);
    let deposit = auction.deposit(&mut ctx);
    ctx.warp_to_timestamp(ESCAPE_TIME - 1);
    let accounts = auction.payout_accounts(&deposit, deposit.depositor_ata);
    ctx.send_transaction(refund_ix(&accounts), &[&auction.admin]).unwrap();
}

#[test]
fn test_refund_requires_the_admin() {
    let mut ctx = TestContext::new();
    let auction = Auction::new(&mut ctx, true);
    auction.allow_mint(&mut ctx);
    let deposit = auction.deposit(&mut ctx);
    let stranger = ctx.create_funded_keypair();
    let mut accounts = auction.payout_accounts(&deposit, deposit.depositor_ata);
    accounts.admin = stranger.pubkey();
    let error = ctx.send_transaction_expect_error(refund_ix(&accounts), &[&stranger]);
    assert_escrow_error(error, EscrowError::InvalidAdmin);
}

#[test]
fn test_refund_cannot_pay_another_address() {
    let mut ctx = TestContext::new();
    let auction = Auction::new(&mut ctx, true);
    auction.allow_mint(&mut ctx);
    let deposit = auction.deposit(&mut ctx);
    let accounts = auction.payout_accounts(&deposit, auction.beneficiary_ata);
    let error = ctx.send_transaction_expect_error(refund_ix(&accounts), &[&auction.admin]);
    assert_escrow_error(error, EscrowError::InvalidPayoutRecipient);
    assert_eq!(ctx.get_token_balance(&auction.vault), AMOUNT);
}

#[test]
fn test_refund_rent_must_return_to_the_depositor() {
    let mut ctx = TestContext::new();
    let auction = Auction::new(&mut ctx, true);
    auction.allow_mint(&mut ctx);
    let deposit = auction.deposit(&mut ctx);
    let mut accounts = auction.payout_accounts(&deposit, deposit.depositor_ata);
    accounts.rent_recipient = auction.admin.pubkey();
    let error = ctx.send_transaction_expect_error(refund_ix(&accounts), &[&auction.admin]);
    assert_escrow_error(error, EscrowError::InvalidRentRecipient);
}

#[test]
fn test_refund_needs_settlement_terms() {
    let mut ctx = TestContext::new();
    let auction = Auction::new(&mut ctx, false);
    auction.allow_mint(&mut ctx);
    let deposit = auction.deposit(&mut ctx);
    let accounts = auction.payout_accounts(&deposit, deposit.depositor_ata);
    let error = ctx.send_transaction_expect_error(refund_ix(&accounts), &[&auction.admin]);
    assert_escrow_error(error, EscrowError::SettlementNotSet);
}

#[test]
fn test_refund_twice_fails_because_the_receipt_is_closed() {
    let mut ctx = TestContext::new();
    let auction = Auction::new(&mut ctx, true);
    auction.allow_mint(&mut ctx);
    let deposit = auction.deposit(&mut ctx);
    let accounts = auction.payout_accounts(&deposit, deposit.depositor_ata);
    ctx.send_transaction(refund_ix(&accounts), &[&auction.admin]).unwrap();
    ctx.expire_blockhash();
    let error = ctx.send_transaction_expect_error(refund_ix(&accounts), &[&auction.admin]);
    assert!(matches!(error, solana_sdk::transaction::TransactionError::InstructionError(..)));
}

// ============================================================================
// Settle
// ============================================================================

#[test]
fn test_settle_pays_the_beneficiary_and_returns_rent_to_the_depositor() {
    let mut ctx = TestContext::new();
    let auction = Auction::new(&mut ctx, true);
    auction.allow_mint(&mut ctx);
    let deposit = auction.deposit(&mut ctx);
    let rent_before = ctx.get_account(&deposit.depositor.pubkey()).unwrap().lamports;
    let receipt_rent = ctx.get_account(&deposit.receipt).unwrap().lamports;
    let accounts = auction.payout_accounts(&deposit, auction.beneficiary_ata);
    ctx.send_transaction(settle_ix(&accounts), &[&auction.admin]).unwrap();
    assert_eq!(ctx.get_token_balance(&auction.beneficiary_ata), AMOUNT);
    assert_eq!(ctx.get_token_balance(&deposit.depositor_ata), AMOUNT);
    assert_eq!(ctx.get_token_balance(&auction.vault), 0);
    assert_account_not_exists(&ctx, &deposit.receipt);
    let rent_after = ctx.get_account(&deposit.depositor.pubkey()).unwrap().lamports;
    assert_eq!(rent_after, rent_before + receipt_rent);
}

#[test]
fn test_settle_cannot_pay_the_depositor() {
    let mut ctx = TestContext::new();
    let auction = Auction::new(&mut ctx, true);
    auction.allow_mint(&mut ctx);
    let deposit = auction.deposit(&mut ctx);
    let accounts = auction.payout_accounts(&deposit, deposit.depositor_ata);
    let error = ctx.send_transaction_expect_error(settle_ix(&accounts), &[&auction.admin]);
    assert_escrow_error(error, EscrowError::InvalidPayoutRecipient);
}

#[test]
fn test_settle_requires_the_admin() {
    let mut ctx = TestContext::new();
    let auction = Auction::new(&mut ctx, true);
    auction.allow_mint(&mut ctx);
    let deposit = auction.deposit(&mut ctx);
    let stranger = ctx.create_funded_keypair();
    let mut accounts = auction.payout_accounts(&deposit, auction.beneficiary_ata);
    accounts.admin = stranger.pubkey();
    let error = ctx.send_transaction_expect_error(settle_ix(&accounts), &[&stranger]);
    assert_escrow_error(error, EscrowError::InvalidAdmin);
}

#[test]
fn test_settle_without_the_admin_signature_fails() {
    let mut ctx = TestContext::new();
    let auction = Auction::new(&mut ctx, true);
    auction.allow_mint(&mut ctx);
    let deposit = auction.deposit(&mut ctx);
    let accounts = auction.payout_accounts(&deposit, auction.beneficiary_ata);
    let mut ix = settle_ix(&accounts);
    ix.accounts[1].is_signer = false;
    let error = ctx.send_transaction_expect_error(ix, &[]);
    assert_instruction_error(error, InstructionError::MissingRequiredSignature);
}

#[test]
fn test_settle_needs_settlement_terms() {
    let mut ctx = TestContext::new();
    let auction = Auction::new(&mut ctx, false);
    auction.allow_mint(&mut ctx);
    let deposit = auction.deposit(&mut ctx);
    let accounts = auction.payout_accounts(&deposit, auction.beneficiary_ata);
    let error = ctx.send_transaction_expect_error(settle_ix(&accounts), &[&auction.admin]);
    assert_escrow_error(error, EscrowError::SettlementNotSet);
}

// ============================================================================
// Events
// ============================================================================

/// The event of a payout: the self call data is the event tag, the event number, then the fields.
fn payout_event(inner: &[Vec<u8>], number: u8) -> Vec<u8> {
    let tag_len = airspace_escrow::events::EVENT_IX_TAG_LE.len();
    let event =
        inner.iter().find(|data| data.len() > tag_len && data[tag_len] == number).expect("the payout emits its event");
    event[tag_len + 1..].to_vec()
}

fn assert_payout_event(fields: &[u8], accounts: &PayoutAccounts, depositor: &Pubkey, recipient: &Pubkey) {
    assert_eq!(fields.len(), 32 * 5 + 8);
    assert_eq!(&fields[0..32], accounts.escrow.as_ref());
    assert_eq!(&fields[32..64], accounts.receipt.as_ref());
    assert_eq!(&fields[64..96], depositor.as_ref());
    assert_eq!(&fields[96..128], recipient.as_ref());
    assert_eq!(&fields[128..160], accounts.mint.as_ref());
    assert_eq!(u64::from_le_bytes(fields[160..168].try_into().unwrap()), AMOUNT);
}

#[test]
fn test_refund_emits_its_event() {
    let mut ctx = TestContext::new();
    let auction = Auction::new(&mut ctx, true);
    auction.allow_mint(&mut ctx);
    let deposit = auction.deposit(&mut ctx);
    let accounts = auction.payout_accounts(&deposit, deposit.depositor_ata);
    let inner = ctx.send_transaction_inner_data(refund_ix(&accounts), &[&auction.admin]);
    let fields = payout_event(&inner, 14);
    assert_payout_event(&fields, &accounts, &deposit.depositor.pubkey(), &deposit.depositor_ata);
}

#[test]
fn test_settle_emits_its_event() {
    let mut ctx = TestContext::new();
    let auction = Auction::new(&mut ctx, true);
    auction.allow_mint(&mut ctx);
    let deposit = auction.deposit(&mut ctx);
    let accounts = auction.payout_accounts(&deposit, auction.beneficiary_ata);
    let inner = ctx.send_transaction_inner_data(settle_ix(&accounts), &[&auction.admin]);
    let fields = payout_event(&inner, 15);
    assert_payout_event(&fields, &accounts, &deposit.depositor.pubkey(), &auction.beneficiary_ata);
}

#[test]
fn test_set_settlement_emits_its_event() {
    let mut ctx = TestContext::new();
    let auction = Auction::new(&mut ctx, false);
    let ix = auction.terms_instruction(&ctx, &auction.admin, DEPOSIT_DEADLINE, ESCAPE_TIME);
    let inner = ctx.send_transaction_inner_data(ix, &[&auction.admin]);
    let fields = payout_event(&inner, 13);
    assert_eq!(&fields[0..32], auction.escrow.as_ref());
    assert_eq!(&fields[32..64], auction.beneficiary.as_ref());
    assert_eq!(i64::from_le_bytes(fields[64..72].try_into().unwrap()), DEPOSIT_DEADLINE);
    assert_eq!(i64::from_le_bytes(fields[72..80].try_into().unwrap()), ESCAPE_TIME);
}
