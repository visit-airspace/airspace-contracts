use solana_sdk::signature::Signer;

use crate::{client, harness::*};
use airspace_deposit::errors::DepositProgramError as E;

fn escape_ix(bid: &PlacedBid) -> solana_sdk::instruction::Instruction {
    client::escape(&bid.bidder.keypair.pubkey(), &bid.bidder.token_account, &bid.accounts)
}

#[test]
fn test_escape_is_refused_before_the_escape_time() {
    let mut world = World::new();
    let bid = world.place(200, 100);
    world.warp(ESCAPE_TIME - 1);
    assert_deposit_error(world.send_err(escape_ix(&bid), &[&bid.bidder.keypair]), E::EscapeTimeNotReached);
    assert_eq!(world.balance(&world.vault), usd(20));
}

#[test]
fn test_escape_returns_the_deposit_at_the_escape_time() {
    let mut world = World::new();
    let bid = world.place(200, 100);
    world.warp(ESCAPE_TIME);
    let before = world.lamports(&bid.bidder.keypair.pubkey());
    let rent = world.lamports(&bid.accounts.bid);
    world.send_ok(escape_ix(&bid), &[&bid.bidder.keypair]);
    assert_eq!(world.balance(&bid.bidder.token_account), usd(200));
    assert_eq!(world.balance(&world.vault), 0);
    assert!(!world.exists(&bid.accounts.bid));
    assert_eq!(world.lamports(&bid.bidder.keypair.pubkey()), before + rent);
}

#[test]
fn test_escape_works_when_the_admin_never_declares_a_winner() {
    let mut world = World::new();
    let first = world.place(200, 100);
    let second = world.place(200, 150);
    world.warp(ESCAPE_TIME + 10 * DAY);
    world.send_ok(escape_ix(&first), &[&first.bidder.keypair]);
    world.send_ok(escape_ix(&second), &[&second.bidder.keypair]);
    assert_eq!(world.balance(&world.vault), 0);
}

#[test]
fn test_escape_is_refused_for_a_winner() {
    let mut world = World::new();
    let bid = world.place(200, 100);
    world.declare(&bid);
    world.warp(ESCAPE_TIME + 1);
    assert_deposit_error(world.send_err(escape_ix(&bid), &[&bid.bidder.keypair]), E::InvalidBidState);
}

#[test]
fn test_escape_is_refused_for_a_paid_bid() {
    let mut world = World::new();
    let bid = world.place(200, 100);
    world.declare(&bid);
    world.pay(&bid);
    world.warp(ESCAPE_TIME + 1);
    assert_deposit_error(world.send_err(escape_ix(&bid), &[&bid.bidder.keypair]), E::InvalidBidState);
    assert_eq!(world.balance(&world.vault), usd(100));
}

#[test]
fn test_escape_is_refused_for_another_signer() {
    let mut world = World::new();
    let bid = world.place(200, 100);
    world.warp(ESCAPE_TIME);
    let other = world.new_bidder(0);
    let ix = client::escape(&other.keypair.pubkey(), &other.token_account, &bid.accounts);
    assert_deposit_error(world.send_err(ix, &[&other.keypair]), E::InvalidBidder);
}

#[test]
fn test_escape_pays_only_the_bidders_own_token_account() {
    let mut world = World::new();
    let bid = world.place(200, 100);
    world.warp(ESCAPE_TIME);
    let other = world.new_bidder(0);
    let ix = client::escape(&bid.bidder.keypair.pubkey(), &other.token_account, &bid.accounts);
    assert_deposit_error(world.send_err(ix, &[&bid.bidder.keypair]), E::InvalidRecipient);
}

#[test]
fn test_a_bid_that_was_refunded_cannot_escape() {
    let mut world = World::new();
    let bid = world.place(200, 100);
    let admin = world.admin.insecure_clone();
    world.send_ok(
        client::refund_deposit(&admin.pubkey(), &bid.bidder.keypair.pubkey(), &bid.bidder.token_account, &bid.accounts),
        &[&admin],
    );
    world.warp(ESCAPE_TIME);
    let error = world.send_err(escape_ix(&bid), &[&bid.bidder.keypair]);
    assert!(matches!(error, solana_sdk::transaction::TransactionError::InstructionError(..)));
    assert_eq!(world.balance(&bid.bidder.token_account), usd(200));
}
