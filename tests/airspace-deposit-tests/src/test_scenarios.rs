//! Whole auctions of the deposit-and-balance model, with every balance checked.

use solana_sdk::signature::Signer;

use crate::{client, harness::*};

/// Two bidders deposit, the outbid one is refunded, the winner pays the remainder, and the sale
/// settles to the treasury.
#[test]
fn test_an_auction_that_ends_in_a_sale() {
    let mut world = World::new();
    let low = world.place(500, 150);
    let high = world.place(500, 301);
    // deposits: 20 percent of 150 is 30, 20 percent of 301 is 60.2 rounded up to 61
    assert_eq!(world.balance(&world.vault), usd(30 + 61));

    world.warp(BID_DEADLINE + 1);
    let admin = world.admin.insecure_clone();
    world.send_ok(
        client::refund_deposit(&admin.pubkey(), &low.bidder.keypair.pubkey(), &low.bidder.token_account, &low.accounts),
        &[&admin],
    );
    assert_eq!(world.balance(&low.bidder.token_account), usd(500));
    assert_eq!(world.balance(&world.vault), usd(61));

    world.send_ok(client::declare_winner(&admin.pubkey(), &high.accounts), &[&admin]);
    world.pay(&high);
    assert_eq!(world.balance(&high.bidder.token_account), usd(500 - 301));
    assert_eq!(world.balance(&world.vault), usd(301));

    world.send_ok(client::settle(&high.bidder.keypair.pubkey(), &world.treasury, &high.accounts), &[]);
    assert_eq!(world.balance(&world.treasury), usd(301));
    assert_eq!(world.balance(&world.vault), 0);
    assert!(!world.exists(&low.accounts.bid));
    assert!(!world.exists(&high.accounts.bid));
}

/// The winner never pays, and the deposit is forfeited to the treasury.
#[test]
fn test_an_auction_that_ends_in_a_forfeit() {
    let mut world = World::new();
    let low = world.place(500, 150);
    let high = world.place(500, 301);
    world.declare(&high);
    let admin = world.admin.insecure_clone();
    world.send_ok(
        client::refund_deposit(&admin.pubkey(), &low.bidder.keypair.pubkey(), &low.bidder.token_account, &low.accounts),
        &[&admin],
    );
    let deadline = world.bid_state(&high).remainder_deadline;
    world.warp(deadline + 1);
    world.send_ok(client::forfeit(&high.bidder.keypair.pubkey(), &world.treasury, &high.accounts), &[]);
    assert_eq!(world.balance(&world.treasury), usd(61));
    assert_eq!(world.balance(&high.bidder.token_account), usd(500 - 61));
    assert_eq!(world.balance(&low.bidder.token_account), usd(500));
    assert_eq!(world.balance(&world.vault), 0);
}

/// The admin disappears after the bid deadline: every bidder gets the deposit back on their own.
#[test]
fn test_an_auction_whose_admin_disappears() {
    let mut world = World::new();
    let bids: Vec<PlacedBid> = (0..3).map(|n| world.place(500, 100 + n * 10)).collect();
    world.warp(ESCAPE_TIME);
    for bid in &bids {
        let ix = client::escape(&bid.bidder.keypair.pubkey(), &bid.bidder.token_account, &bid.accounts);
        world.send_ok(ix, &[&bid.bidder.keypair]);
        assert_eq!(world.balance(&bid.bidder.token_account), usd(500));
    }
    assert_eq!(world.balance(&world.vault), 0);
    assert_eq!(world.balance(&world.treasury), 0);
}

/// A winner who was declared and paid cannot be pulled back out of the vault by escaping, and the
/// money reaches the treasury through Settle, which anyone may send after the escape time too.
#[test]
fn test_a_paid_winner_is_settled_even_after_the_escape_time() {
    let mut world = World::new();
    let bid = world.place(500, 200);
    world.declare(&bid);
    world.pay(&bid);
    world.warp(ESCAPE_TIME + 5 * DAY);
    world.send_ok(client::settle(&bid.bidder.keypair.pubkey(), &world.treasury, &bid.accounts), &[]);
    assert_eq!(world.balance(&world.treasury), usd(200));
}

/// The vault always holds exactly the money of the bids that still exist.
#[test]
fn test_the_vault_balance_always_matches_the_live_bids() {
    let mut world = World::new();
    let a = world.place(500, 100);
    let b = world.place(500, 205);
    let c = world.place(500, 50);
    let held = |world: &World| [&a, &b, &c].iter().filter(|x| world.exists(&x.accounts.bid)).count();
    assert_eq!(held(&world), 3);
    assert_eq!(world.balance(&world.vault), usd(20 + 41 + 10));
    world.declare(&b);
    let admin = world.admin.insecure_clone();
    world.send_ok(
        client::refund_deposit(&admin.pubkey(), &a.bidder.keypair.pubkey(), &a.bidder.token_account, &a.accounts),
        &[&admin],
    );
    assert_eq!(world.balance(&world.vault), usd(41 + 10));
    world.send_ok(
        client::refund_deposit(&admin.pubkey(), &c.bidder.keypair.pubkey(), &c.bidder.token_account, &c.accounts),
        &[&admin],
    );
    assert_eq!(world.balance(&world.vault), usd(41));
    world.pay(&b);
    assert_eq!(world.balance(&world.vault), usd(205));
}
