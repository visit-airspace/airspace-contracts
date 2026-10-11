use solana_sdk::signature::Signer;

use crate::{client, harness::*};
use airspace_deposit::{errors::DepositProgramError as E, state::BidState};

fn declare_ix(world: &World, bid: &PlacedBid) -> solana_sdk::instruction::Instruction {
    client::declare_winner(&world.admin.pubkey(), &bid.accounts)
}

#[test]
fn test_declare_winner_starts_the_payment_window() {
    let mut world = World::new();
    let bid = world.place(200, 100);
    world.warp(BID_DEADLINE + 100);
    let admin = world.admin.insecure_clone();
    world.send_ok(declare_ix(&world, &bid), &[&admin]);
    let state = world.bid_state(&bid);
    assert_eq!(state.state, BidState::Winner);
    assert_eq!(state.remainder_deadline, BID_DEADLINE + 100 + PAYMENT_WINDOW);
    assert_eq!(world.auction_state().winner_bid.as_ref(), bid.accounts.bid.as_ref());
}

#[test]
fn test_declare_winner_is_refused_before_the_bid_deadline() {
    let mut world = World::new();
    let bid = world.place(200, 100);
    world.warp(BID_DEADLINE);
    let admin = world.admin.insecure_clone();
    assert_deposit_error(world.send_err(declare_ix(&world, &bid), &[&admin]), E::BidDeadlineNotReached);
}

#[test]
fn test_a_second_declaration_is_refused() {
    let mut world = World::new();
    let first = world.place(200, 100);
    let second = world.place(200, 150);
    world.declare(&first);
    let admin = world.admin.insecure_clone();
    assert_deposit_error(world.send_err(declare_ix(&world, &second), &[&admin]), E::WinnerAlreadyDeclared);
    assert_deposit_error(world.send_err(declare_ix(&world, &first), &[&admin]), E::WinnerAlreadyDeclared);
}

#[test]
fn test_declare_winner_requires_the_admin() {
    let mut world = World::new();
    let bid = world.place(200, 100);
    world.warp(BID_DEADLINE + 1);
    let stranger = world.new_bidder(0);
    let ix = client::declare_winner(&stranger.keypair.pubkey(), &bid.accounts);
    assert_deposit_error(world.send_err(ix, &[&stranger.keypair]), E::InvalidAdmin);
}

#[test]
fn test_the_winner_must_still_be_deposited() {
    let mut world = World::new();
    let bid = world.place(200, 100);
    let admin = world.admin.insecure_clone();
    world.send_ok(
        client::refund_deposit(&admin.pubkey(), &bid.bidder.keypair.pubkey(), &bid.bidder.token_account, &bid.accounts),
        &[&admin],
    );
    world.warp(BID_DEADLINE + 1);
    // A refunded bid no longer exists, so it cannot win.
    let error = world.send_err(declare_ix(&world, &bid), &[&admin]);
    assert!(matches!(error, solana_sdk::transaction::TransactionError::InstructionError(..)));
}

#[test]
fn test_a_declaration_whose_window_would_pass_the_escape_time_is_refused() {
    let mut world = World::new();
    let bid = world.place(200, 100);
    // The window of 48 hours must end before the escape time.
    world.warp(ESCAPE_TIME - PAYMENT_WINDOW);
    let admin = world.admin.insecure_clone();
    assert_deposit_error(world.send_err(declare_ix(&world, &bid), &[&admin]), E::PaymentWindowPassesEscape);
}

#[test]
fn test_a_declaration_just_inside_the_escape_time_is_accepted() {
    let mut world = World::new();
    let bid = world.place(200, 100);
    world.warp(ESCAPE_TIME - PAYMENT_WINDOW - 1);
    let admin = world.admin.insecure_clone();
    world.send_ok(declare_ix(&world, &bid), &[&admin]);
}
