use solana_sdk::signature::Signer;

use crate::{client, harness::*};
use airspace_deposit::{errors::DepositProgramError as E, state::BidState};

fn pay_ix(bid: &PlacedBid) -> solana_sdk::instruction::Instruction {
    client::pay_remainder(&bid.bidder.keypair.pubkey(), &bid.bidder.token_account, &bid.accounts)
}

#[test]
fn test_pay_remainder_moves_exactly_the_bid_minus_the_deposit() {
    let mut world = World::new();
    let bid = world.place(200, 101);
    world.declare(&bid);
    // deposit 21, so the remainder is 80
    world.send_ok(pay_ix(&bid), &[&bid.bidder.keypair]);
    assert_eq!(world.balance(&bid.bidder.token_account), usd(200 - 101));
    assert_eq!(world.balance(&world.vault), usd(101));
    let state = world.bid_state(&bid);
    assert_eq!(state.state, BidState::Paid);
    assert_eq!(state.remainder_paid, usd(80));
    assert_eq!(state.deposit + state.remainder_paid, state.amount);
}

#[test]
fn test_pay_remainder_is_refused_before_the_declaration() {
    let mut world = World::new();
    let bid = world.place(200, 100);
    world.warp(BID_DEADLINE + 1);
    assert_deposit_error(world.send_err(pay_ix(&bid), &[&bid.bidder.keypair]), E::InvalidBidState);
}

#[test]
fn test_pay_remainder_is_refused_for_a_bid_that_did_not_win() {
    let mut world = World::new();
    let winner = world.place(200, 100);
    let other = world.place(200, 100);
    world.declare(&winner);
    assert_deposit_error(world.send_err(pay_ix(&other), &[&other.bidder.keypair]), E::InvalidBidState);
}

#[test]
fn test_pay_remainder_is_allowed_at_the_deadline_and_refused_after() {
    let mut world = World::new();
    let late = world.place(200, 100);
    world.declare(&late);
    let deadline = world.bid_state(&late).remainder_deadline;
    world.warp(deadline + 1);
    assert_deposit_error(world.send_err(pay_ix(&late), &[&late.bidder.keypair]), E::RemainderDeadlinePassed);

    let mut world = World::new();
    let on_time = world.place(200, 100);
    world.declare(&on_time);
    let deadline = world.bid_state(&on_time).remainder_deadline;
    world.warp(deadline);
    world.send_ok(pay_ix(&on_time), &[&on_time.bidder.keypair]);
}

#[test]
fn test_pay_remainder_is_refused_for_anyone_but_the_winner() {
    let mut world = World::new();
    let winner = world.place(200, 100);
    world.declare(&winner);
    let other = world.new_bidder(500);
    let ix = client::pay_remainder(&other.keypair.pubkey(), &other.token_account, &winner.accounts);
    assert_deposit_error(world.send_err(ix, &[&other.keypair]), E::InvalidBidder);
}

#[test]
fn test_the_remainder_cannot_be_paid_twice() {
    let mut world = World::new();
    let bid = world.place(300, 100);
    world.declare(&bid);
    world.pay(&bid);
    world.svm.expire_blockhash();
    assert_deposit_error(world.send_err(pay_ix(&bid), &[&bid.bidder.keypair]), E::InvalidBidState);
    assert_eq!(world.balance(&world.vault), usd(100));
}

#[test]
fn test_pay_remainder_needs_the_bidder_signature() {
    let mut world = World::new();
    let bid = world.place(200, 100);
    world.declare(&bid);
    let mut ix = pay_ix(&bid);
    ix.accounts[0].is_signer = false;
    let error = world.send_err(ix, &[]);
    assert_instruction_error(error, solana_sdk::instruction::InstructionError::MissingRequiredSignature);
}

#[test]
fn test_a_bid_of_the_minimum_has_nothing_left_to_pay() {
    let mut world = World::new();
    let bid = world.place(100, 10);
    world.declare(&bid);
    world.send_ok(pay_ix(&bid), &[&bid.bidder.keypair]);
    assert_eq!(world.bid_state(&bid).remainder_paid, 0);
    assert_eq!(world.balance(&world.vault), usd(10));
}
