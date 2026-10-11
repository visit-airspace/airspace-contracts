//! Hand built instructions for the AirSpace deposit program. The account order and the data layout
//! of each instruction are the ones that `docs/DEPOSIT_PROGRAM.md` documents.

use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};

pub const PROGRAM_ID: Pubkey = airspace_deposit::ID;
pub const SYSTEM_PROGRAM_ID: Pubkey = solana_system_interface::program::ID;
pub const TOKEN_PROGRAM_ID: Pubkey = spl_token_interface::ID;
pub const ASSOCIATED_TOKEN_PROGRAM_ID: Pubkey = spl_associated_token_account::ID;

pub fn event_authority() -> Pubkey {
    Pubkey::find_program_address(&[b"event_authority"], &PROGRAM_ID).0
}

pub fn auction_address(admin: &Pubkey, auction_id: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"auction", admin.as_ref(), auction_id.as_ref()], &PROGRAM_ID).0
}

pub fn bid_address(auction: &Pubkey, bidder: &Pubkey, bid_seed: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"bid", auction.as_ref(), bidder.as_ref(), bid_seed.as_ref()], &PROGRAM_ID).0
}

fn tail(mut accounts: Vec<AccountMeta>) -> Vec<AccountMeta> {
    accounts.push(AccountMeta::new_readonly(event_authority(), false));
    accounts.push(AccountMeta::new_readonly(PROGRAM_ID, false));
    accounts
}

/// The terms that CreateAuction takes.
#[derive(Clone, Copy)]
pub struct Terms {
    pub auction_id: Pubkey,
    pub deposit_bps: u16,
    pub min_deposit: u64,
    pub bid_deadline: i64,
    pub payment_window: i64,
    pub escape_time: i64,
}

pub struct Accounts {
    pub admin: Pubkey,
    pub auction: Pubkey,
    pub mint: Pubkey,
    pub treasury: Pubkey,
    pub vault: Pubkey,
}

pub fn create_auction(a: &Accounts, terms: &Terms) -> Instruction {
    let mut data = vec![0u8];
    data.extend_from_slice(terms.auction_id.as_ref());
    data.extend_from_slice(&terms.deposit_bps.to_le_bytes());
    data.extend_from_slice(&terms.min_deposit.to_le_bytes());
    data.extend_from_slice(&terms.bid_deadline.to_le_bytes());
    data.extend_from_slice(&terms.payment_window.to_le_bytes());
    data.extend_from_slice(&terms.escape_time.to_le_bytes());
    let accounts = tail(vec![
        AccountMeta::new(a.admin, true),
        AccountMeta::new(a.auction, false),
        AccountMeta::new_readonly(a.mint, false),
        AccountMeta::new_readonly(a.treasury, false),
        AccountMeta::new(a.vault, false),
        AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
        AccountMeta::new_readonly(ASSOCIATED_TOKEN_PROGRAM_ID, false),
        AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
    ]);
    // event_authority and program come last, after the system program
    Instruction { program_id: PROGRAM_ID, accounts, data }
}

pub struct BidAccounts {
    pub auction: Pubkey,
    pub bid: Pubkey,
    pub vault: Pubkey,
    pub mint: Pubkey,
}

pub fn place_deposit(
    bidder: &Pubkey,
    bidder_token_account: &Pubkey,
    b: &BidAccounts,
    bid_seed: &Pubkey,
    amount: u64,
) -> Instruction {
    let mut data = vec![1u8];
    data.extend_from_slice(bid_seed.as_ref());
    data.extend_from_slice(&amount.to_le_bytes());
    let accounts = tail(vec![
        AccountMeta::new(*bidder, true),
        AccountMeta::new_readonly(b.auction, false),
        AccountMeta::new(b.bid, false),
        AccountMeta::new(b.vault, false),
        AccountMeta::new(*bidder_token_account, false),
        AccountMeta::new_readonly(b.mint, false),
        AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
        AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
    ]);
    Instruction { program_id: PROGRAM_ID, accounts, data }
}

pub fn refund_deposit(admin: &Pubkey, bidder: &Pubkey, bidder_token_account: &Pubkey, b: &BidAccounts) -> Instruction {
    let accounts = tail(vec![
        AccountMeta::new_readonly(*admin, true),
        AccountMeta::new(*bidder, false),
        AccountMeta::new_readonly(b.auction, false),
        AccountMeta::new(b.bid, false),
        AccountMeta::new(b.vault, false),
        AccountMeta::new(*bidder_token_account, false),
        AccountMeta::new_readonly(b.mint, false),
        AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
    ]);
    Instruction { program_id: PROGRAM_ID, accounts, data: vec![2u8] }
}

pub fn declare_winner(admin: &Pubkey, b: &BidAccounts) -> Instruction {
    let accounts = tail(vec![
        AccountMeta::new_readonly(*admin, true),
        AccountMeta::new(b.auction, false),
        AccountMeta::new(b.bid, false),
    ]);
    Instruction { program_id: PROGRAM_ID, accounts, data: vec![3u8] }
}

pub fn pay_remainder(bidder: &Pubkey, bidder_token_account: &Pubkey, b: &BidAccounts) -> Instruction {
    let accounts = tail(vec![
        AccountMeta::new_readonly(*bidder, true),
        AccountMeta::new_readonly(b.auction, false),
        AccountMeta::new(b.bid, false),
        AccountMeta::new(b.vault, false),
        AccountMeta::new(*bidder_token_account, false),
        AccountMeta::new_readonly(b.mint, false),
        AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
    ]);
    Instruction { program_id: PROGRAM_ID, accounts, data: vec![4u8] }
}

fn to_treasury(discriminator: u8, bidder: &Pubkey, treasury: &Pubkey, b: &BidAccounts) -> Instruction {
    let accounts = tail(vec![
        AccountMeta::new(*bidder, false),
        AccountMeta::new_readonly(b.auction, false),
        AccountMeta::new(b.bid, false),
        AccountMeta::new(b.vault, false),
        AccountMeta::new(*treasury, false),
        AccountMeta::new_readonly(b.mint, false),
        AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
    ]);
    Instruction { program_id: PROGRAM_ID, accounts, data: vec![discriminator] }
}

pub fn settle(bidder: &Pubkey, treasury: &Pubkey, b: &BidAccounts) -> Instruction {
    to_treasury(5, bidder, treasury, b)
}

pub fn forfeit(bidder: &Pubkey, treasury: &Pubkey, b: &BidAccounts) -> Instruction {
    to_treasury(6, bidder, treasury, b)
}

pub fn escape(bidder: &Pubkey, bidder_token_account: &Pubkey, b: &BidAccounts) -> Instruction {
    let accounts = tail(vec![
        AccountMeta::new(*bidder, true),
        AccountMeta::new_readonly(b.auction, false),
        AccountMeta::new(b.bid, false),
        AccountMeta::new(b.vault, false),
        AccountMeta::new(*bidder_token_account, false),
        AccountMeta::new_readonly(b.mint, false),
        AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
    ]);
    Instruction { program_id: PROGRAM_ID, accounts, data: vec![7u8] }
}
