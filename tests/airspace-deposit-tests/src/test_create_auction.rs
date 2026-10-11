use solana_sdk::{pubkey::Pubkey, signature::Signer};

use crate::{
    client::{self, TOKEN_PROGRAM_ID},
    harness::*,
};
use airspace_deposit::errors::DepositProgramError as E;

#[test]
fn test_create_auction_stores_the_fixed_terms() {
    let world = World::new();
    let state = world.auction_state();
    assert_eq!(state.admin, world.admin.pubkey().to_bytes().into());
    assert_eq!(state.auction_id.as_ref(), world.auction_id.as_ref());
    assert_eq!(state.mint.as_ref(), world.mint.as_ref());
    assert_eq!(state.treasury.as_ref(), world.treasury.as_ref());
    assert_eq!(state.vault.as_ref(), world.vault.as_ref());
    assert_eq!(state.deposit_bps, DEPOSIT_BPS);
    assert_eq!(state.min_deposit, MIN_DEPOSIT);
    assert_eq!(state.bid_deadline, BID_DEADLINE);
    assert_eq!(state.payment_window, PAYMENT_WINDOW);
    assert_eq!(state.escape_time, ESCAPE_TIME);
    assert!(!state.has_winner());
}

#[test]
fn test_create_auction_makes_the_vault_owned_by_the_auction() {
    let world = World::new();
    let vault = world.svm.get_account(&world.vault).expect("the vault exists");
    assert_eq!(vault.owner, TOKEN_PROGRAM_ID);
    assert_eq!(&vault.data[0..32], world.mint.as_ref());
    assert_eq!(&vault.data[32..64], world.auction.as_ref());
    assert_eq!(world.balance(&world.vault), 0);
}

fn create_with(change: impl FnOnce(&mut client::Terms)) -> Result<(), solana_sdk::transaction::TransactionError> {
    let mut world = World::without_auction();
    let mut terms = world.terms();
    change(&mut terms);
    let ix = client::create_auction(&world.auction_accounts(), &terms);
    let admin = world.admin.insecure_clone();
    world.send(ix, &[&admin]).map(|_| ())
}

#[test]
fn test_deposit_share_below_1000_is_refused() {
    assert_deposit_error(create_with(|t| t.deposit_bps = 900).unwrap_err(), E::InvalidDepositShare);
    assert_deposit_error(create_with(|t| t.deposit_bps = 999).unwrap_err(), E::InvalidDepositShare);
    assert_deposit_error(create_with(|t| t.deposit_bps = 0).unwrap_err(), E::InvalidDepositShare);
}

#[test]
fn test_deposit_share_above_5000_is_refused() {
    assert_deposit_error(create_with(|t| t.deposit_bps = 5100).unwrap_err(), E::InvalidDepositShare);
    assert_deposit_error(create_with(|t| t.deposit_bps = 10_000).unwrap_err(), E::InvalidDepositShare);
}

#[test]
fn test_deposit_share_must_be_a_multiple_of_100() {
    assert_deposit_error(create_with(|t| t.deposit_bps = 1050).unwrap_err(), E::InvalidDepositShare);
    assert_deposit_error(create_with(|t| t.deposit_bps = 2001).unwrap_err(), E::InvalidDepositShare);
}

#[test]
fn test_deposit_share_limits_are_accepted() {
    create_with(|t| t.deposit_bps = 1000).unwrap();
    create_with(|t| t.deposit_bps = 5000).unwrap();
    create_with(|t| t.deposit_bps = 3300).unwrap();
}

#[test]
fn test_minimum_deposit_must_be_a_whole_positive_dollar_amount() {
    assert_deposit_error(create_with(|t| t.min_deposit = 0).unwrap_err(), E::InvalidMinimumDeposit);
    assert_deposit_error(create_with(|t| t.min_deposit = DOLLAR + 1).unwrap_err(), E::InvalidMinimumDeposit);
}

#[test]
fn test_bid_deadline_must_be_in_the_future() {
    assert_deposit_error(create_with(|t| t.bid_deadline = START).unwrap_err(), E::InvalidAuctionTerms);
}

#[test]
fn test_payment_window_must_be_positive() {
    assert_deposit_error(create_with(|t| t.payment_window = 0).unwrap_err(), E::InvalidAuctionTerms);
}

#[test]
fn test_escape_time_must_be_seven_days_after_the_payment_window() {
    assert_deposit_error(create_with(|t| t.escape_time = ESCAPE_TIME - 1).unwrap_err(), E::InvalidAuctionTerms);
    create_with(|t| t.escape_time = ESCAPE_TIME).unwrap();
}

#[test]
fn test_an_auction_cannot_be_created_twice() {
    let mut world = World::new();
    let ix = client::create_auction(&world.auction_accounts(), &world.terms());
    let admin = world.admin.insecure_clone();
    let error = world.send_err(ix, &[&admin]);
    assert!(matches!(error, solana_sdk::transaction::TransactionError::InstructionError(..)));
}

#[test]
fn test_a_mint_without_6_decimals_is_refused() {
    let mut world = World::without_auction();
    world.set_mint(world.mint, 9);
    let ix = client::create_auction(&world.auction_accounts(), &world.terms());
    let admin = world.admin.insecure_clone();
    assert_deposit_error(world.send_err(ix, &[&admin]), E::InvalidMint);
}

#[test]
fn test_a_treasury_of_another_mint_is_refused() {
    let mut world = World::without_auction();
    let other = Pubkey::new_unique();
    world.set_mint(other, 6);
    let owner = world.treasury_owner;
    world.treasury = world.set_token_account(&owner, &other, 0);
    let ix = client::create_auction(&world.auction_accounts(), &world.terms());
    let admin = world.admin.insecure_clone();
    assert_deposit_error(world.send_err(ix, &[&admin]), E::InvalidMint);
}

#[test]
fn test_the_auction_account_must_match_its_seeds() {
    let mut world = World::without_auction();
    world.auction = Pubkey::new_unique();
    let ix = client::create_auction(&world.auction_accounts(), &world.terms());
    let admin = world.admin.insecure_clone();
    assert_deposit_error(world.send_err(ix, &[&admin]), E::InvalidSeeds);
}

#[test]
fn test_the_vault_must_be_the_associated_token_account_of_the_auction() {
    let mut world = World::without_auction();
    world.vault = Pubkey::new_unique();
    let ix = client::create_auction(&world.auction_accounts(), &world.terms());
    let admin = world.admin.insecure_clone();
    assert_deposit_error(world.send_err(ix, &[&admin]), E::InvalidVault);
}

#[test]
fn test_create_auction_needs_the_admin_signature() {
    let mut world = World::without_auction();
    let mut ix = client::create_auction(&world.auction_accounts(), &world.terms());
    ix.accounts[0].is_signer = false;
    let error = world.send_err(ix, &[]);
    assert_instruction_error(error, solana_sdk::instruction::InstructionError::MissingRequiredSignature);
}

#[test]
fn test_a_mint_with_token_extensions_is_refused() {
    let mut world = World::without_auction();
    // An extended mint is longer than the 82 bytes of a plain mint.
    let mut account = world.svm.get_account(&world.mint).unwrap();
    account.data.resize(200, 0);
    world.svm.set_account(world.mint, account).unwrap();
    let ix = client::create_auction(&world.auction_accounts(), &world.terms());
    let admin = world.admin.insecure_clone();
    assert_deposit_error(world.send_err(ix, &[&admin]), E::MintHasExtensions);
}

#[test]
fn test_a_treasury_that_is_not_a_token_account_is_refused() {
    let mut world = World::without_auction();
    world.treasury = Pubkey::new_unique();
    let ix = client::create_auction(&world.auction_accounts(), &world.terms());
    let admin = world.admin.insecure_clone();
    let error = world.send_err(ix, &[&admin]);
    assert!(matches!(error, solana_sdk::transaction::TransactionError::InstructionError(..)));
}

#[test]
fn test_a_treasury_that_is_not_an_associated_token_account_is_refused() {
    let mut world = World::without_auction();
    // A token account of the right mint and owner, at an address that nobody could recreate
    // through the associated token program if it were closed.
    let address = Pubkey::new_unique();
    let (owner, mint) = (world.treasury_owner, world.mint);
    world.set_token_account_at(address, &owner, &mint, 0);
    world.treasury = address;
    let ix = client::create_auction(&world.auction_accounts(), &world.terms());
    let admin = world.admin.insecure_clone();
    assert_deposit_error(world.send_err(ix, &[&admin]), E::InvalidTreasury);
}

#[test]
fn test_a_treasury_with_token_extensions_is_refused() {
    let mut world = World::without_auction();
    // An extended token account, for example one that requires memos on transfers in, is longer
    // than the 165 bytes of a plain token account.
    let mut account = world.svm.get_account(&world.treasury).unwrap();
    account.data.resize(200, 0);
    world.svm.set_account(world.treasury, account).unwrap();
    let ix = client::create_auction(&world.auction_accounts(), &world.terms());
    let admin = world.admin.insecure_clone();
    assert_deposit_error(world.send_err(ix, &[&admin]), E::InvalidTreasury);
}
