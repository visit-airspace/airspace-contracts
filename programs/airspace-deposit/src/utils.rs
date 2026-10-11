use pinocchio::{
    account::AccountView,
    cpi::{invoke_signed, Seed, Signer},
    error::ProgramError,
    instruction::{InstructionAccount, InstructionView},
    sysvars::{rent::Rent, Sysvar},
    Address, ProgramResult,
};
use pinocchio_associated_token_account::ID as ASSOCIATED_TOKEN_ACCOUNT_PROGRAM_ID;
use pinocchio_system::instructions::{Allocate, Assign, CreateAccount, Transfer};
use pinocchio_token_2022::{instructions::TransferChecked, ID as TOKEN_2022_PROGRAM_ID};

use crate::{
    errors::DepositProgramError,
    events::{event_authority_pda, EVENT_AUTHORITY_SEED},
    state::{Auction, Bid, AUCTION_SEED, BID_SEED},
};

pub use pinocchio_token::ID as TOKEN_PROGRAM_ID;

/// Length of an SPL mint without extensions, and the offsets that the program reads from it.
const MINT_LEN: usize = 82;
const MINT_DECIMALS_OFFSET: usize = 44;
const MINT_INITIALIZED_OFFSET: usize = 45;
/// Length of an SPL token account without extensions, and the offsets that the program reads from it.
const TOKEN_ACCOUNT_LEN: usize = 165;
const TOKEN_ACCOUNT_MINT_OFFSET: usize = 0;
const TOKEN_ACCOUNT_OWNER_OFFSET: usize = 32;
const TOKEN_ACCOUNT_STATE_OFFSET: usize = 108;
const TOKEN_ACCOUNT_INITIALIZED: u8 = 1;

/// Length of a Token-2022 associated token account whose only extension is the ImmutableOwner
/// that the associated token program always adds: the base account, one account type byte, and
/// a four byte extension header with no data.
const TOKEN_2022_ASSOCIATED_ACCOUNT_LEN: usize = 170;

/// Decimals of the one mint that the program accepts, so one dollar is 1,000,000 base units.
pub const MINT_DECIMALS: u8 = 6;

#[inline(always)]
pub fn verify_signer(account: &AccountView, expect_writable: bool) -> Result<(), ProgramError> {
    if !account.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }
    verify_writable(account, expect_writable)
}

#[inline(always)]
pub fn verify_writable(account: &AccountView, expect_writable: bool) -> Result<(), ProgramError> {
    if !expect_writable {
        return Ok(());
    }
    if !account.is_writable() {
        return Err(ProgramError::Immutable);
    }
    Ok(())
}

#[inline(always)]
pub fn verify_readonly(account: &AccountView) -> Result<(), ProgramError> {
    if account.is_writable() {
        return Err(ProgramError::AccountBorrowFailed);
    }
    Ok(())
}

#[inline(always)]
pub fn verify_owned_by(account: &AccountView, owner: &Address) -> Result<(), ProgramError> {
    if !account.owned_by(owner) {
        return Err(ProgramError::InvalidAccountOwner);
    }
    Ok(())
}

#[inline(always)]
pub fn verify_system_program(account: &AccountView) -> Result<(), ProgramError> {
    if account.address() != &pinocchio_system::ID {
        return Err(ProgramError::IncorrectProgramId);
    }
    Ok(())
}

#[inline(always)]
pub fn verify_associated_token_program(account: &AccountView) -> Result<(), ProgramError> {
    if account.address() != &ASSOCIATED_TOKEN_ACCOUNT_PROGRAM_ID {
        return Err(ProgramError::IncorrectProgramId);
    }
    Ok(())
}

#[inline(always)]
pub fn verify_token_program(account: &AccountView) -> Result<(), ProgramError> {
    if account.address() == &TOKEN_PROGRAM_ID {
        return Ok(());
    }
    if account.address() == &TOKEN_2022_PROGRAM_ID {
        return Ok(());
    }
    Err(ProgramError::IncorrectProgramId)
}

#[inline(always)]
pub fn verify_current_program(program_id: &Address, account: &AccountView) -> Result<(), ProgramError> {
    if account.address() != program_id {
        return Err(ProgramError::IncorrectProgramId);
    }
    Ok(())
}

#[inline(always)]
pub fn verify_event_authority(account: &AccountView) -> Result<(), ProgramError> {
    if account.address() != &event_authority_pda::ID {
        return Err(DepositProgramError::InvalidEventAuthority.into());
    }
    Ok(())
}

/// Verifies that the mint belongs to the token program, is initialized, has 6 decimals, and
/// carries no token extensions, which means its length is the base length of a mint.
pub fn verify_mint(mint: &AccountView, token_program: &AccountView) -> Result<(), ProgramError> {
    verify_owned_by(mint, token_program.address())?;
    if mint.data_len() != MINT_LEN {
        return Err(DepositProgramError::MintHasExtensions.into());
    }
    let data = mint.try_borrow()?;
    if data[MINT_INITIALIZED_OFFSET] != 1 {
        return Err(ProgramError::UninitializedAccount);
    }
    if data[MINT_DECIMALS_OFFSET] != MINT_DECIMALS {
        return Err(DepositProgramError::InvalidMint.into());
    }
    Ok(())
}

/// Verifies that the account is an initialized token account of the token program for the mint.
pub fn verify_token_account(
    account: &AccountView,
    mint: &Address,
    token_program: &AccountView,
) -> Result<(), ProgramError> {
    verify_owned_by(account, token_program.address())?;
    if account.data_len() < TOKEN_ACCOUNT_LEN {
        return Err(ProgramError::InvalidAccountData);
    }
    let data = account.try_borrow()?;
    if data[TOKEN_ACCOUNT_STATE_OFFSET] != TOKEN_ACCOUNT_INITIALIZED {
        return Err(ProgramError::UninitializedAccount);
    }
    if &data[TOKEN_ACCOUNT_MINT_OFFSET..TOKEN_ACCOUNT_MINT_OFFSET + 32] != mint.as_ref() {
        return Err(DepositProgramError::InvalidMint.into());
    }
    Ok(())
}

/// Verifies that a new auction's treasury is the associated token account of its own owner for
/// the mint, and that it carries no token extensions besides the ImmutableOwner that the
/// associated token program adds. A paid bid can only end in Settle, which pays this one fixed
/// address, so the treasury must stay payable: anyone can recreate an associated token account
/// that was closed, and an account without extensions cannot require memos on transfers in.
pub fn verify_new_treasury(
    treasury: &AccountView,
    mint: &AccountView,
    token_program: &AccountView,
) -> Result<(), ProgramError> {
    verify_token_account(treasury, mint.address(), token_program)?;
    let expected_len = if token_program.address() == &TOKEN_2022_PROGRAM_ID {
        TOKEN_2022_ASSOCIATED_ACCOUNT_LEN
    } else {
        TOKEN_ACCOUNT_LEN
    };
    if treasury.data_len() != expected_len {
        return Err(DepositProgramError::InvalidTreasury.into());
    }
    let owner = {
        let data = treasury.try_borrow()?;
        let bytes: [u8; 32] = data[TOKEN_ACCOUNT_OWNER_OFFSET..TOKEN_ACCOUNT_OWNER_OFFSET + 32]
            .try_into()
            .map_err(|_| ProgramError::InvalidAccountData)?;
        Address::new_from_array(bytes)
    };
    if treasury.address() != &associated_token_address(&owner, mint.address(), token_program.address()) {
        return Err(DepositProgramError::InvalidTreasury.into());
    }
    Ok(())
}

/// The associated token account address of a wallet for a mint.
pub fn associated_token_address(wallet: &Address, mint: &Address, token_program: &Address) -> Address {
    Address::find_program_address(
        &[wallet.as_ref(), token_program.as_ref(), mint.as_ref()],
        &ASSOCIATED_TOKEN_ACCOUNT_PROGRAM_ID,
    )
    .0
}

/// Verifies that the account is the associated token account of the wallet, and that it exists.
pub fn verify_recipient(
    account: &AccountView,
    wallet: &Address,
    mint: &AccountView,
    token_program: &AccountView,
) -> Result<(), ProgramError> {
    let expected = associated_token_address(wallet, mint.address(), token_program.address());
    if account.address() != &expected {
        return Err(DepositProgramError::InvalidRecipient.into());
    }
    verify_token_account(account, mint.address(), token_program)
}

/// The address of an auction account.
pub fn auction_address(program_id: &Address, admin: &Address, auction_id: &Address) -> (Address, u8) {
    Address::find_program_address(&[AUCTION_SEED, admin.as_ref(), auction_id.as_ref()], program_id)
}

/// The address of a bid account.
pub fn bid_address(program_id: &Address, auction: &Address, bidder: &Address, bid_seed: &Address) -> (Address, u8) {
    Address::find_program_address(&[BID_SEED, auction.as_ref(), bidder.as_ref(), bid_seed.as_ref()], program_id)
}

/// Reads an auction account: it must be owned by this program and sit at the seeds of its own
/// admin and id.
pub fn load_auction(program_id: &Address, account: &AccountView) -> Result<Auction, ProgramError> {
    verify_owned_by(account, program_id)?;
    let auction = {
        let data = account.try_borrow()?;
        Auction::from_bytes(&data)?
    };
    if account.address() != &auction_address(program_id, &auction.admin, &auction.auction_id).0 {
        return Err(DepositProgramError::InvalidSeeds.into());
    }
    Ok(auction)
}

/// Reads a bid account: it must be owned by this program, sit at the seeds of its own auction,
/// bidder, and seed, and belong to the auction that the instruction names.
pub fn load_bid(program_id: &Address, auction: &AccountView, account: &AccountView) -> Result<Bid, ProgramError> {
    verify_owned_by(account, program_id)?;
    let bid = {
        let data = account.try_borrow()?;
        Bid::from_bytes(&data)?
    };
    if &bid.auction != auction.address() {
        return Err(DepositProgramError::InvalidBidAuction.into());
    }
    if account.address() != &bid_address(program_id, &bid.auction, &bid.bidder, &bid.bid_seed).0 {
        return Err(DepositProgramError::InvalidSeeds.into());
    }
    Ok(bid)
}

/// Writes the serialized bytes of an account into it.
pub fn write_account(account: &AccountView, bytes: &[u8]) -> ProgramResult {
    let mut data = account.try_borrow_mut()?;
    if data.len() != bytes.len() {
        return Err(ProgramError::InvalidAccountData);
    }
    data.copy_from_slice(bytes);
    Ok(())
}

/// Close a PDA account and return the lamports to the recipient.
pub fn close_pda_account(pda_account: &AccountView, recipient: &AccountView) -> ProgramResult {
    if pda_account.address() == recipient.address() {
        return Err(ProgramError::InvalidArgument);
    }
    let recipient_lamports = recipient.lamports();
    let recipient_balance =
        recipient_lamports.checked_add(pda_account.lamports()).ok_or(ProgramError::ArithmeticOverflow)?;
    recipient.set_lamports(recipient_balance);
    pda_account.set_lamports(0);
    pda_account.close()?;
    assert_eq!(recipient.lamports(), recipient_balance, "closing the bid lost the recipient's rent refund");
    assert_eq!(pda_account.data_len(), 0, "closed bid retained account data");
    Ok(())
}

/// Create a PDA account for the given seeds, once: an account that already has data is refused.
pub fn create_pda_account<const N: usize>(
    payer: &AccountView,
    space: usize,
    owner: &Address,
    pda_account: &AccountView,
    pda_signer_seeds: [Seed; N],
) -> ProgramResult {
    let rent = Rent::get()?;
    let required_lamports = rent.try_minimum_balance(space)?.max(1);
    let signers = [Signer::from(&pda_signer_seeds)];

    if pda_account.lamports() > 0 {
        if pda_account.data_len() > 0 {
            return Err(ProgramError::AccountAlreadyInitialized);
        }
        // The PDA was prefunded but never initialized.
        let additional_lamports = required_lamports.saturating_sub(pda_account.lamports());
        if additional_lamports > 0 {
            Transfer { from: payer, to: pda_account, lamports: additional_lamports }.invoke()?;
        }
        Allocate { account: pda_account, space: space as u64 }.invoke_signed(&signers)?;
        Assign { account: pda_account, owner }.invoke_signed(&signers)?;
    } else {
        CreateAccount { from: payer, to: pda_account, lamports: required_lamports, space: space as u64, owner }
            .invoke_signed(&signers)?;
    }
    assert_eq!(pda_account.data_len(), space, "created auction PDA has the wrong allocation size");
    assert!(pda_account.owned_by(owner), "created auction PDA was not assigned to its program");
    assert!(pda_account.lamports() >= required_lamports, "created auction PDA is not funded for rent exemption");
    Ok(())
}

/// Moves tokens with `transfer_checked`, signed by the wallet that owns the source.
pub fn transfer_from_wallet(
    from: &AccountView,
    to: &AccountView,
    mint: &AccountView,
    authority: &AccountView,
    token_program: &AccountView,
    amount: u64,
) -> ProgramResult {
    TransferChecked {
        from,
        mint,
        to,
        authority,
        amount,
        decimals: MINT_DECIMALS,
        token_program: token_program.address(),
    }
    .invoke()
}

/// Moves tokens out of the auction's vault, signed by the auction PDA.
pub fn transfer_from_vault(
    auction: &AccountView,
    state: &Auction,
    vault: &AccountView,
    to: &AccountView,
    mint: &AccountView,
    token_program: &AccountView,
    amount: u64,
) -> ProgramResult {
    let bump = [state.bump];
    let seeds: [Seed; 4] = [
        Seed::from(AUCTION_SEED),
        Seed::from(state.admin.as_ref()),
        Seed::from(state.auction_id.as_ref()),
        Seed::from(bump.as_slice()),
    ];
    let signers = [Signer::from(&seeds)];
    TransferChecked {
        from: vault,
        mint,
        to,
        authority: auction,
        amount,
        decimals: MINT_DECIMALS,
        token_program: token_program.address(),
    }
    .invoke_signed(&signers)
}

/// Verifies the vault against the auction, and the mint and token program with it.
pub fn verify_vault(
    state: &Auction,
    vault: &AccountView,
    mint: &AccountView,
    token_program: &AccountView,
) -> Result<(), ProgramError> {
    if vault.address() != &state.vault {
        return Err(DepositProgramError::InvalidVault.into());
    }
    verify_mint_of_auction(state, mint, token_program)?;
    verify_token_account(vault, &state.mint, token_program)
}

/// Verifies that the mint is the auction's mint and a plain 6 decimal mint of the token program.
pub fn verify_mint_of_auction(
    state: &Auction,
    mint: &AccountView,
    token_program: &AccountView,
) -> Result<(), ProgramError> {
    verify_token_program(token_program)?;
    if mint.address() != &state.mint {
        return Err(DepositProgramError::InvalidMint.into());
    }
    verify_mint(mint, token_program)
}

/// Verifies the treasury token account against the auction.
pub fn verify_treasury(
    state: &Auction,
    treasury: &AccountView,
    token_program: &AccountView,
) -> Result<(), ProgramError> {
    if treasury.address() != &state.treasury {
        return Err(DepositProgramError::InvalidTreasury.into());
    }
    verify_token_account(treasury, &state.mint, token_program)
}

/// Emits an event via CPI to prevent log truncation.
pub fn emit_event(
    program_id: &Address,
    event_authority: &AccountView,
    program: &AccountView,
    event_data: &[u8],
) -> ProgramResult {
    verify_event_authority(event_authority)?;
    verify_current_program(program_id, program)?;

    let bump = [event_authority_pda::BUMP];
    let signer_seeds: [Seed; 2] = [Seed::from(EVENT_AUTHORITY_SEED), Seed::from(&bump)];
    let signer = Signer::from(&signer_seeds);
    let accounts = [InstructionAccount::readonly_signer(event_authority.address())];
    let instruction = InstructionView { program_id, accounts: &accounts, data: event_data };
    invoke_signed(&instruction, &[event_authority, program], &[signer])
}

/// The data of an instruction that has no fields after its discriminator.
pub fn require_empty(data: &[u8]) -> ProgramResult {
    if !data.is_empty() {
        return Err(ProgramError::InvalidInstructionData);
    }
    Ok(())
}

/// Splits `len` bytes off the front of the instruction data, and refuses leftovers.
pub fn exact(data: &[u8], len: usize) -> Result<&[u8], ProgramError> {
    if data.len() != len {
        return Err(ProgramError::InvalidInstructionData);
    }
    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pinocchio::account::{RuntimeAccount, NOT_BORROWED};

    #[test]
    fn test_close_rejects_an_aliased_rent_recipient_without_mutation() {
        let mut raw = RuntimeAccount {
            borrow_state: NOT_BORROWED,
            address: Address::new_from_array([1; 32]),
            owner: crate::ID,
            lamports: 25,
            ..RuntimeAccount::default()
        };
        // The raw account remains live and has an empty data region.
        let account = unsafe { AccountView::new_unchecked(&mut raw) };
        assert_eq!(close_pda_account(&account, &account), Err(ProgramError::InvalidArgument));
        assert_eq!(account.lamports(), 25);
        assert!(account.owned_by(&crate::ID));
    }

    #[test]
    fn test_close_overflow_preserves_both_accounts() {
        let mut source_raw = RuntimeAccount {
            borrow_state: NOT_BORROWED,
            address: Address::new_from_array([1; 32]),
            owner: crate::ID,
            lamports: 25,
            ..RuntimeAccount::default()
        };
        let mut recipient_raw = RuntimeAccount {
            borrow_state: NOT_BORROWED,
            address: Address::new_from_array([2; 32]),
            lamports: u64::MAX,
            ..RuntimeAccount::default()
        };
        // Both raw accounts remain live and have empty data regions.
        let source = unsafe { AccountView::new_unchecked(&mut source_raw) };
        let recipient = unsafe { AccountView::new_unchecked(&mut recipient_raw) };
        assert_eq!(close_pda_account(&source, &recipient), Err(ProgramError::ArithmeticOverflow));
        assert_eq!(source.lamports(), 25);
        assert_eq!(recipient.lamports(), u64::MAX);
        assert!(source.owned_by(&crate::ID));
    }
}
