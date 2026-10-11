use solana_sdk::{pubkey::Pubkey, signature::Signer};

use crate::{client, harness::*};
use airspace_deposit::{errors::DepositProgramError as E, state::BidState};

fn place_error(world: &mut World, wallet: u64, amount: u64) -> solana_sdk::transaction::TransactionError {
    let bidder = world.new_bidder(wallet);
    let seed = Pubkey::new_unique();
    let accounts = world.bid_accounts(&bidder.keypair.pubkey(), &seed);
    let ix = client::place_deposit(&bidder.keypair.pubkey(), &bidder.token_account, &accounts, &seed, amount);
    world.send_err(ix, &[&bidder.keypair])
}

#[test]
fn test_deposit_is_the_share_rounded_up_to_a_whole_dollar() {
    let mut world = World::new();
    // 20 percent of 101 dollars is 20.2 dollars, which rounds up to 21
    let bid = world.place(200, 101);
    assert_eq!(world.bid_state(&bid).deposit, usd(21));
    assert_eq!(world.balance(&world.vault), usd(21));
    assert_eq!(world.balance(&bid.bidder.token_account), usd(200 - 21));
    // 20 percent of 100 dollars is exactly 20
    let bid = world.place(200, 100);
    assert_eq!(world.bid_state(&bid).deposit, usd(20));
}

#[test]
fn test_deposit_is_never_below_the_minimum() {
    let mut world = World::new();
    // 20 percent of 20 dollars is 4, below the minimum of 10
    let bid = world.place(100, 20);
    assert_eq!(world.bid_state(&bid).deposit, MIN_DEPOSIT);
    assert_eq!(world.balance(&world.vault), MIN_DEPOSIT);
}

#[test]
fn test_a_bid_at_the_minimum_deposits_the_whole_bid() {
    let mut world = World::new();
    let bid = world.place(100, 10);
    assert_eq!(world.bid_state(&bid).deposit, usd(10));
    assert_eq!(world.bid_state(&bid).amount, usd(10));
}

#[test]
fn test_the_bid_is_created_in_state_deposited() {
    let mut world = World::new();
    let bid = world.place(200, 100);
    let state = world.bid_state(&bid);
    assert_eq!(state.state, BidState::Deposited);
    assert_eq!(state.amount, usd(100));
    assert_eq!(state.remainder_paid, 0);
    assert_eq!(state.remainder_deadline, 0);
    assert_eq!(state.bidder.as_ref(), bid.bidder.keypair.pubkey().as_ref());
    assert_eq!(state.auction.as_ref(), world.auction.as_ref());
    assert_eq!(state.bid_seed.as_ref(), bid.seed.as_ref());
}

#[test]
fn test_the_bidder_pays_the_rent_of_the_bid() {
    let mut world = World::new();
    let bidder = world.new_bidder(200);
    let before = world.lamports(&bidder.keypair.pubkey());
    let seed = Pubkey::new_unique();
    let accounts = world.bid_accounts(&bidder.keypair.pubkey(), &seed);
    let ix = client::place_deposit(&bidder.keypair.pubkey(), &bidder.token_account, &accounts, &seed, usd(100));
    world.send_ok(ix, &[&bidder.keypair]);
    let rent = world.lamports(&accounts.bid);
    assert!(rent > 0);
    assert_eq!(world.lamports(&bidder.keypair.pubkey()), before - rent);
}

#[test]
fn test_deposit_is_allowed_at_the_bid_deadline_and_refused_after() {
    let mut world = World::new();
    world.warp(BID_DEADLINE);
    world.place(200, 100);
    world.warp(BID_DEADLINE + 1);
    let error = place_error(&mut world, 200, usd(100));
    assert_deposit_error(error, E::BidDeadlinePassed);
}

#[test]
fn test_zero_amount_is_refused() {
    let mut world = World::new();
    assert_deposit_error(place_error(&mut world, 200, 0), E::InvalidBidAmount);
}

#[test]
fn test_amount_that_is_not_whole_dollars_is_refused() {
    let mut world = World::new();
    assert_deposit_error(place_error(&mut world, 200, usd(100) + 1), E::InvalidBidAmount);
}

#[test]
fn test_amount_below_the_minimum_is_refused() {
    let mut world = World::new();
    assert_deposit_error(place_error(&mut world, 200, usd(9)), E::InvalidBidAmount);
}

#[test]
fn test_another_mint_is_refused() {
    let mut world = World::new();
    let other = Pubkey::new_unique();
    world.set_mint(other, 6);
    let bidder = world.new_bidder(200);
    let other_account = world.set_token_account(&bidder.keypair.pubkey(), &other, usd(200));
    let seed = Pubkey::new_unique();
    let mut accounts = world.bid_accounts(&bidder.keypair.pubkey(), &seed);
    accounts.mint = other;
    let ix = client::place_deposit(&bidder.keypair.pubkey(), &other_account, &accounts, &seed, usd(100));
    assert_deposit_error(world.send_err(ix, &[&bidder.keypair]), E::InvalidMint);
}

#[test]
fn test_another_vault_is_refused() {
    let mut world = World::new();
    let bidder = world.new_bidder(200);
    let mint = world.mint;
    let other_vault = world.set_token_account(&Pubkey::new_unique(), &mint, 0);
    let seed = Pubkey::new_unique();
    let mut accounts = world.bid_accounts(&bidder.keypair.pubkey(), &seed);
    accounts.vault = other_vault;
    let ix = client::place_deposit(&bidder.keypair.pubkey(), &bidder.token_account, &accounts, &seed, usd(100));
    assert_deposit_error(world.send_err(ix, &[&bidder.keypair]), E::InvalidVault);
}

#[test]
fn test_the_bid_account_must_match_its_seeds() {
    let mut world = World::new();
    let bidder = world.new_bidder(200);
    let seed = Pubkey::new_unique();
    let mut accounts = world.bid_accounts(&bidder.keypair.pubkey(), &seed);
    accounts.bid = Pubkey::new_unique();
    let ix = client::place_deposit(&bidder.keypair.pubkey(), &bidder.token_account, &accounts, &seed, usd(100));
    assert_deposit_error(world.send_err(ix, &[&bidder.keypair]), E::InvalidSeeds);
}

#[test]
fn test_the_same_bid_seed_cannot_be_used_twice() {
    let mut world = World::new();
    let bid = world.place(300, 100);
    let ix = client::place_deposit(
        &bid.bidder.keypair.pubkey(),
        &bid.bidder.token_account,
        &bid.accounts,
        &bid.seed,
        usd(100),
    );
    let error = world.send_err(ix, &[&bid.bidder.keypair]);
    assert!(matches!(error, solana_sdk::transaction::TransactionError::InstructionError(..)));
}

#[test]
fn test_a_deposit_without_the_bidder_signature_fails() {
    let mut world = World::new();
    let bidder = world.new_bidder(200);
    let seed = Pubkey::new_unique();
    let accounts = world.bid_accounts(&bidder.keypair.pubkey(), &seed);
    let mut ix = client::place_deposit(&bidder.keypair.pubkey(), &bidder.token_account, &accounts, &seed, usd(100));
    ix.accounts[0].is_signer = false;
    let error = world.send_err(ix, &[]);
    assert_instruction_error(error, solana_sdk::instruction::InstructionError::MissingRequiredSignature);
}

#[test]
fn test_a_bidder_cannot_deposit_from_a_wallet_that_did_not_sign() {
    let mut world = World::new();
    let victim = world.new_bidder(500);
    let attacker = world.new_bidder(0);
    let seed = Pubkey::new_unique();
    let accounts = world.bid_accounts(&attacker.keypair.pubkey(), &seed);
    // The attacker signs, but the source is the victim's token account.
    let ix = client::place_deposit(&attacker.keypair.pubkey(), &victim.token_account, &accounts, &seed, usd(100));
    let error = world.send_err(ix, &[&attacker.keypair]);
    assert!(matches!(error, solana_sdk::transaction::TransactionError::InstructionError(..)));
    assert_eq!(world.balance(&victim.token_account), usd(500));
}
