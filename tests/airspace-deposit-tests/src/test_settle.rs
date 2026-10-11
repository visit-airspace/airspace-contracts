use solana_sdk::{pubkey::Pubkey, signature::Signer};

use crate::{client, harness::*};
use airspace_deposit::errors::DepositProgramError as E;

fn settle_ix(world: &World, bid: &PlacedBid) -> solana_sdk::instruction::Instruction {
    client::settle(&bid.bidder.keypair.pubkey(), &world.treasury, &bid.accounts)
}

#[test]
fn test_settle_moves_the_whole_bid_to_the_treasury_and_closes_the_bid() {
    let mut world = World::new();
    let bid = world.place(200, 100);
    world.declare(&bid);
    world.pay(&bid);
    let before = world.lamports(&bid.bidder.keypair.pubkey());
    let rent = world.lamports(&bid.accounts.bid);
    // Anyone may send it: only the fee payer signs.
    world.send_ok(settle_ix(&world, &bid), &[]);
    assert_eq!(world.balance(&world.treasury), usd(100));
    assert_eq!(world.balance(&world.vault), 0);
    assert!(!world.exists(&bid.accounts.bid));
    assert_eq!(world.lamports(&bid.bidder.keypair.pubkey()), before + rent);
}

#[test]
fn test_settle_is_refused_for_an_unpaid_bid() {
    let mut world = World::new();
    let deposited = world.place(200, 100);
    assert_deposit_error(world.send_err(settle_ix(&world, &deposited), &[]), E::InvalidBidState);
    world.declare(&deposited);
    assert_deposit_error(world.send_err(settle_ix(&world, &deposited), &[]), E::InvalidBidState);
}

#[test]
fn test_settle_pays_only_the_auction_treasury() {
    let mut world = World::new();
    let bid = world.place(200, 100);
    world.declare(&bid);
    world.pay(&bid);
    let mint = world.mint;
    let other = world.set_token_account(&Pubkey::new_unique(), &mint, 0);
    let ix = client::settle(&bid.bidder.keypair.pubkey(), &other, &bid.accounts);
    assert_deposit_error(world.send_err(ix, &[]), E::InvalidTreasury);
    assert_eq!(world.balance(&other), 0);
}

#[test]
fn test_settle_returns_the_rent_to_the_bidder_only() {
    let mut world = World::new();
    let bid = world.place(200, 100);
    world.declare(&bid);
    world.pay(&bid);
    let ix = client::settle(&world.payer.pubkey(), &world.treasury, &bid.accounts);
    assert_deposit_error(world.send_err(ix, &[]), E::InvalidBidder);
}

#[test]
fn test_a_settled_bid_cannot_be_settled_again() {
    let mut world = World::new();
    let bid = world.place(200, 100);
    world.declare(&bid);
    world.pay(&bid);
    world.send_ok(settle_ix(&world, &bid), &[]);
    world.svm.expire_blockhash();
    let error = world.send_err(settle_ix(&world, &bid), &[]);
    assert!(matches!(error, solana_sdk::transaction::TransactionError::InstructionError(..)));
    assert_eq!(world.balance(&world.treasury), usd(100));
}
