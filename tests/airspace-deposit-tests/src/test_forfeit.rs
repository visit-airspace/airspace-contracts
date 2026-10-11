use solana_sdk::{pubkey::Pubkey, signature::Signer};

use crate::{client, harness::*};
use airspace_deposit::errors::DepositProgramError as E;

fn forfeit_ix(world: &World, bid: &PlacedBid) -> solana_sdk::instruction::Instruction {
    client::forfeit(&bid.bidder.keypair.pubkey(), &world.treasury, &bid.accounts)
}

#[test]
fn test_forfeit_moves_only_the_deposit_to_the_treasury_after_the_deadline() {
    let mut world = World::new();
    let bid = world.place(200, 100);
    world.declare(&bid);
    let deadline = world.bid_state(&bid).remainder_deadline;
    world.warp(deadline + 1);
    let before = world.lamports(&bid.bidder.keypair.pubkey());
    let rent = world.lamports(&bid.accounts.bid);
    world.send_ok(forfeit_ix(&world, &bid), &[]);
    assert_eq!(world.balance(&world.treasury), usd(20));
    assert_eq!(world.balance(&world.vault), 0);
    assert_eq!(world.balance(&bid.bidder.token_account), usd(180));
    assert!(!world.exists(&bid.accounts.bid));
    assert_eq!(world.lamports(&bid.bidder.keypair.pubkey()), before + rent);
}

#[test]
fn test_forfeit_is_refused_until_the_deadline_has_passed() {
    let mut world = World::new();
    let bid = world.place(200, 100);
    world.declare(&bid);
    let deadline = world.bid_state(&bid).remainder_deadline;
    world.warp(deadline);
    assert_deposit_error(world.send_err(forfeit_ix(&world, &bid), &[]), E::RemainderDeadlineNotPassed);
}

#[test]
fn test_forfeit_is_refused_for_a_paid_bid() {
    let mut world = World::new();
    let bid = world.place(200, 100);
    world.declare(&bid);
    world.pay(&bid);
    let deadline = world.bid_state(&bid).remainder_deadline;
    world.warp(deadline + 1);
    assert_deposit_error(world.send_err(forfeit_ix(&world, &bid), &[]), E::InvalidBidState);
}

#[test]
fn test_forfeit_is_refused_for_a_bid_that_was_never_declared() {
    let mut world = World::new();
    let bid = world.place(200, 100);
    world.warp(ESCAPE_TIME + DAY);
    assert_deposit_error(world.send_err(forfeit_ix(&world, &bid), &[]), E::InvalidBidState);
}

#[test]
fn test_forfeit_pays_only_the_auction_treasury() {
    let mut world = World::new();
    let bid = world.place(200, 100);
    world.declare(&bid);
    let deadline = world.bid_state(&bid).remainder_deadline;
    world.warp(deadline + 1);
    let mint = world.mint;
    let other = world.set_token_account(&Pubkey::new_unique(), &mint, 0);
    let ix = client::forfeit(&bid.bidder.keypair.pubkey(), &other, &bid.accounts);
    assert_deposit_error(world.send_err(ix, &[]), E::InvalidTreasury);
}

#[test]
fn test_forfeit_leaves_other_bidders_deposits_in_the_vault() {
    let mut world = World::new();
    let winner = world.place(200, 100);
    let other = world.place(200, 100);
    world.declare(&winner);
    let deadline = world.bid_state(&winner).remainder_deadline;
    world.warp(deadline + 1);
    world.send_ok(forfeit_ix(&world, &winner), &[]);
    assert_eq!(world.balance(&world.vault), usd(20));
    assert_eq!(world.bid_state(&other).deposit, usd(20));
}
