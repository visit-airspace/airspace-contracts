use solana_sdk::{pubkey::Pubkey, signature::Signer};

use crate::{client, harness::*};
use airspace_deposit::errors::DepositProgramError as E;

fn refund_ix(world: &World, bid: &PlacedBid) -> solana_sdk::instruction::Instruction {
    client::refund_deposit(
        &world.admin.pubkey(),
        &bid.bidder.keypair.pubkey(),
        &bid.bidder.token_account,
        &bid.accounts,
    )
}

#[test]
fn test_refund_returns_the_whole_deposit_to_the_bidder() {
    let mut world = World::new();
    let bid = world.place(200, 100);
    assert_eq!(world.balance(&bid.bidder.token_account), usd(180));
    let before = world.lamports(&bid.bidder.keypair.pubkey());
    let rent = world.lamports(&bid.accounts.bid);
    let admin = world.admin.insecure_clone();
    world.send_ok(refund_ix(&world, &bid), &[&admin]);
    assert_eq!(world.balance(&bid.bidder.token_account), usd(200));
    assert_eq!(world.balance(&world.vault), 0);
    assert!(!world.exists(&bid.accounts.bid));
    assert_eq!(world.lamports(&bid.bidder.keypair.pubkey()), before + rent);
}

#[test]
fn test_refund_can_run_before_and_after_the_bid_deadline() {
    let mut world = World::new();
    let first = world.place(200, 100);
    let second = world.place(200, 100);
    let admin = world.admin.insecure_clone();
    world.send_ok(refund_ix(&world, &first), &[&admin]);
    world.warp(BID_DEADLINE + 5);
    world.send_ok(refund_ix(&world, &second), &[&admin]);
}

#[test]
fn test_refund_requires_the_admin() {
    let mut world = World::new();
    let bid = world.place(200, 100);
    let stranger = world.new_bidder(0);
    let ix = client::refund_deposit(
        &stranger.keypair.pubkey(),
        &bid.bidder.keypair.pubkey(),
        &bid.bidder.token_account,
        &bid.accounts,
    );
    assert_deposit_error(world.send_err(ix, &[&stranger.keypair]), E::InvalidAdmin);
}

#[test]
fn test_refund_cannot_pay_another_address() {
    let mut world = World::new();
    let bid = world.place(200, 100);
    let thief = world.new_bidder(0);
    let ix = client::refund_deposit(
        &world.admin.pubkey(),
        &bid.bidder.keypair.pubkey(),
        &thief.token_account,
        &bid.accounts,
    );
    let admin = world.admin.insecure_clone();
    assert_deposit_error(world.send_err(ix, &[&admin]), E::InvalidRecipient);
    assert_eq!(world.balance(&world.vault), usd(20));
}

#[test]
fn test_refund_rent_goes_back_to_the_bidder_only() {
    let mut world = World::new();
    let bid = world.place(200, 100);
    let ix =
        client::refund_deposit(&world.admin.pubkey(), &world.admin.pubkey(), &bid.bidder.token_account, &bid.accounts);
    let admin = world.admin.insecure_clone();
    assert_deposit_error(world.send_err(ix, &[&admin]), E::InvalidBidder);
}

#[test]
fn test_refund_is_refused_for_the_declared_winner() {
    let mut world = World::new();
    let bid = world.place(200, 100);
    world.declare(&bid);
    let admin = world.admin.insecure_clone();
    assert_deposit_error(world.send_err(refund_ix(&world, &bid), &[&admin]), E::InvalidBidState);
}

#[test]
fn test_refund_is_refused_for_a_paid_bid() {
    let mut world = World::new();
    let bid = world.place(200, 100);
    world.declare(&bid);
    world.pay(&bid);
    let admin = world.admin.insecure_clone();
    assert_deposit_error(world.send_err(refund_ix(&world, &bid), &[&admin]), E::InvalidBidState);
}

#[test]
fn test_refund_of_a_bid_of_another_auction_is_refused() {
    let mut world = World::new();
    let mut bid = world.place(200, 100);
    bid.accounts.auction = Pubkey::new_unique();
    let admin = world.admin.insecure_clone();
    let error = world.send_err(refund_ix(&world, &bid), &[&admin]);
    assert!(matches!(error, solana_sdk::transaction::TransactionError::InstructionError(..)));
}

#[test]
fn test_refund_without_the_admin_signature_fails() {
    let mut world = World::new();
    let bid = world.place(200, 100);
    let mut ix = refund_ix(&world, &bid);
    ix.accounts[0].is_signer = false;
    let error = world.send_err(ix, &[]);
    assert_instruction_error(error, solana_sdk::instruction::InstructionError::MissingRequiredSignature);
}
