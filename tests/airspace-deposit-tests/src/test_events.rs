use solana_sdk::signature::Signer;

use crate::{client, harness::*};

fn word(fields: &[u8], from: usize) -> [u8; 8] {
    fields[from..from + 8].try_into().unwrap()
}

fn check_bid_event(fields: &[u8], world: &World, bid: &PlacedBid, recipient: &[u8], amount: u64) {
    assert_eq!(fields.len(), 32 * 4 + 8);
    assert_eq!(&fields[0..32], world.auction.as_ref());
    assert_eq!(&fields[32..64], bid.accounts.bid.as_ref());
    assert_eq!(&fields[64..96], bid.bidder.keypair.pubkey().as_ref());
    assert_eq!(&fields[96..128], recipient);
    assert_eq!(u64::from_le_bytes(word(fields, 128)), amount);
}

#[test]
fn test_create_auction_emits_auction_created() {
    let mut world = World::without_auction();
    let ix = client::create_auction(&world.auction_accounts(), &world.terms());
    let admin = world.admin.insecure_clone();
    let inner = world.send_ok(ix, &[&admin]);
    let fields = event_fields(&inner, 0);
    assert_eq!(&fields[0..32], world.auction.as_ref());
    assert_eq!(&fields[32..64], admin.pubkey().as_ref());
    assert_eq!(&fields[64..96], world.mint.as_ref());
    assert_eq!(&fields[96..128], world.treasury.as_ref());
    assert_eq!(u16::from_le_bytes(fields[128..130].try_into().unwrap()), DEPOSIT_BPS);
    assert_eq!(u64::from_le_bytes(word(&fields, 130)), MIN_DEPOSIT);
    assert_eq!(i64::from_le_bytes(word(&fields, 138)), BID_DEADLINE);
    assert_eq!(i64::from_le_bytes(word(&fields, 146)), PAYMENT_WINDOW);
    assert_eq!(i64::from_le_bytes(word(&fields, 154)), ESCAPE_TIME);
}

#[test]
fn test_place_deposit_emits_deposit_placed() {
    let mut world = World::new();
    let bidder = world.new_bidder(200);
    let seed = solana_sdk::pubkey::Pubkey::new_unique();
    let accounts = world.bid_accounts(&bidder.keypair.pubkey(), &seed);
    let ix = client::place_deposit(&bidder.keypair.pubkey(), &bidder.token_account, &accounts, &seed, usd(100));
    let inner = world.send_ok(ix, &[&bidder.keypair]);
    let bid = PlacedBid { bidder, accounts, seed };
    check_bid_event(&event_fields(&inner, 1), &world, &bid, world.vault.as_ref(), usd(20));
}

#[test]
fn test_refund_deposit_emits_deposit_refunded() {
    let mut world = World::new();
    let bid = world.place(200, 100);
    let admin = world.admin.insecure_clone();
    let ix =
        client::refund_deposit(&admin.pubkey(), &bid.bidder.keypair.pubkey(), &bid.bidder.token_account, &bid.accounts);
    let inner = world.send_ok(ix, &[&admin]);
    check_bid_event(&event_fields(&inner, 2), &world, &bid, bid.bidder.token_account.as_ref(), usd(20));
}

#[test]
fn test_declare_winner_emits_winner_declared() {
    let mut world = World::new();
    let bid = world.place(200, 100);
    world.warp(BID_DEADLINE + 1);
    let admin = world.admin.insecure_clone();
    let inner = world.send_ok(client::declare_winner(&admin.pubkey(), &bid.accounts), &[&admin]);
    check_bid_event(&event_fields(&inner, 3), &world, &bid, &[0u8; 32], usd(100));
}

#[test]
fn test_pay_remainder_emits_remainder_paid() {
    let mut world = World::new();
    let bid = world.place(200, 100);
    world.declare(&bid);
    let ix = client::pay_remainder(&bid.bidder.keypair.pubkey(), &bid.bidder.token_account, &bid.accounts);
    let inner = world.send_ok(ix, &[&bid.bidder.keypair]);
    check_bid_event(&event_fields(&inner, 4), &world, &bid, world.vault.as_ref(), usd(80));
}

#[test]
fn test_settle_emits_bid_settled() {
    let mut world = World::new();
    let bid = world.place(200, 100);
    world.declare(&bid);
    world.pay(&bid);
    let ix = client::settle(&bid.bidder.keypair.pubkey(), &world.treasury, &bid.accounts);
    let inner = world.send_ok(ix, &[]);
    check_bid_event(&event_fields(&inner, 5), &world, &bid, world.treasury.as_ref(), usd(100));
}

#[test]
fn test_forfeit_emits_bid_forfeited() {
    let mut world = World::new();
    let bid = world.place(200, 100);
    world.declare(&bid);
    let deadline = world.bid_state(&bid).remainder_deadline;
    world.warp(deadline + 1);
    let ix = client::forfeit(&bid.bidder.keypair.pubkey(), &world.treasury, &bid.accounts);
    let inner = world.send_ok(ix, &[]);
    check_bid_event(&event_fields(&inner, 6), &world, &bid, world.treasury.as_ref(), usd(20));
}

#[test]
fn test_escape_emits_bid_escaped() {
    let mut world = World::new();
    let bid = world.place(200, 100);
    world.warp(ESCAPE_TIME);
    let ix = client::escape(&bid.bidder.keypair.pubkey(), &bid.bidder.token_account, &bid.accounts);
    let inner = world.send_ok(ix, &[&bid.bidder.keypair]);
    check_bid_event(&event_fields(&inner, 7), &world, &bid, bid.bidder.token_account.as_ref(), usd(20));
}

#[test]
fn test_the_event_authority_must_be_the_program_pda() {
    let mut world = World::new();
    let bid = world.place(200, 100);
    world.warp(BID_DEADLINE + 1);
    let admin = world.admin.insecure_clone();
    let mut ix = client::declare_winner(&admin.pubkey(), &bid.accounts);
    ix.accounts[3].pubkey = solana_sdk::pubkey::Pubkey::new_unique();
    let error = world.send_err(ix, &[&admin]);
    assert_deposit_error(error, airspace_deposit::errors::DepositProgramError::InvalidEventAuthority);
}
