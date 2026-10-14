//! Fixed program/client vectors consumed by the Go control service tests.
use airspace_deposit::state::{Auction, Bid, BidState};
use airspace_deposit_tests::client::{self, Accounts, BidAccounts, Terms, PROGRAM_ID};
use solana_sdk::{instruction::Instruction, pubkey::Pubkey};

fn hex(data: &[u8]) -> String {
    data.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn instruction(tag: u8, value: Instruction) {
    let metas: Vec<String> = value.accounts.iter().map(|account| {
        format!("{}:{}:{}", hex(account.pubkey.as_ref()), u8::from(account.is_signer), u8::from(account.is_writable))
    }).collect();
    println!("instruction|{tag}|{}|{}|{}", hex(value.program_id.as_ref()), hex(&value.data), metas.join(","));
}

fn main() {
    let admin = Pubkey::new_from_array([1; 32]);
    let bidder = Pubkey::new_from_array([2; 32]);
    let mint = Pubkey::new_from_array([3; 32]);
    let treasury = Pubkey::new_from_array([4; 32]);
    let auction_id = Pubkey::new_from_array([5; 32]);
    let seed = Pubkey::new_from_array([6; 32]);
    let (auction, auction_bump) = Pubkey::find_program_address(&[b"auction", admin.as_ref(), auction_id.as_ref()], &PROGRAM_ID);
    let (bid, bid_bump) = Pubkey::find_program_address(&[b"bid", auction.as_ref(), bidder.as_ref(), seed.as_ref()], &PROGRAM_ID);
    let vault = spl_associated_token_account::get_associated_token_address(&auction, &mint);
    let bidder_token = spl_associated_token_account::get_associated_token_address(&bidder, &mint);
    let a = Accounts { admin, auction, mint, treasury, vault };
    let b = BidAccounts { auction, bid, vault, mint };
    let terms = Terms { auction_id, deposit_bps: 2000, min_deposit: 10_000_000, bid_deadline: 2_000_000, payment_window: 172800, escape_time: 2_777_600 };
    instruction(0, client::create_auction(&a, &terms));
    instruction(1, client::place_deposit(&bidder, &bidder_token, &b, &seed, 50_000_000));
    instruction(2, client::refund_deposit(&admin, &bidder, &bidder_token, &b));
    instruction(3, client::declare_winner(&admin, &b));
    instruction(4, client::pay_remainder(&bidder, &bidder_token, &b));
    instruction(5, client::settle(&bidder, &treasury, &b));
    instruction(6, client::forfeit(&bidder, &treasury, &b));
    instruction(7, client::escape(&bidder, &bidder_token, &b));
    println!("pda|auction|{}|{auction_bump}", hex(auction.as_ref()));
    println!("pda|bid|{}|{bid_bump}", hex(bid.as_ref()));
    println!("pda|event|{}", hex(client::event_authority().as_ref()));
    let state = Auction { bump: auction_bump, auction_id, admin, mint, treasury, vault, deposit_bps: terms.deposit_bps, min_deposit: terms.min_deposit, bid_deadline: terms.bid_deadline, payment_window: terms.payment_window, escape_time: terms.escape_time, winner_bid: bid };
    let auction_bytes = state.to_bytes();
    assert_eq!(auction_bytes.len(), Auction::LEN);
    println!("state|auction|{}", hex(&auction_bytes));
    let state = Bid { bump: bid_bump, state: BidState::Paid, auction, bidder, bid_seed: seed, amount: 50_000_000, deposit: 10_000_000, remainder_paid: 40_000_000, remainder_deadline: 2_172_800 };
    let bid_bytes = state.to_bytes();
    assert_eq!(bid_bytes.len(), Bid::LEN);
    println!("state|bid|{}", hex(&bid_bytes));
    instruction(8, spl_token_interface::instruction::initialize_mint2(&spl_token_interface::id(), &mint, &admin, None, 6).expect("initialize mint"));
    instruction(9, spl_token_interface::instruction::mint_to_checked(&spl_token_interface::id(), &mint, &bidder_token, &admin, &[], 123_000_000, 6).expect("mint checked"));
    instruction(10, spl_token_interface::instruction::transfer_checked(&spl_token_interface::id(), &bidder_token, &mint, &treasury, &bidder, &[], 123_000_000, 6).expect("transfer checked"));
    instruction(11, spl_associated_token_account::instruction::create_associated_token_account_idempotent(&admin, &bidder, &mint, &spl_token_interface::id()));
}
