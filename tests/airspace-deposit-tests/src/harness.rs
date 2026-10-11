//! A LiteSVM world with one funded auction, and the helpers that the tests share.

use litesvm::LiteSVM;
use solana_account::Account;
use solana_program::{clock::Clock, program_option::COption, program_pack::Pack};
use solana_sdk::{
    instruction::{Instruction, InstructionError},
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::{Transaction, TransactionError},
};
use spl_associated_token_account::get_associated_token_address;
use spl_token_interface::{
    state::{Account as TokenAccount, AccountState, Mint},
    ID as TOKEN_PROGRAM_ID,
};

use crate::client::{self, Accounts, BidAccounts, Terms, PROGRAM_ID};
use airspace_deposit::{
    errors::DepositProgramError,
    state::{Auction, Bid},
};

pub const START: i64 = 1_000_000;
pub const DAY: i64 = 86_400;
pub const HOUR: i64 = 3_600;
pub const BID_DEADLINE: i64 = START + 10 * DAY;
pub const PAYMENT_WINDOW: i64 = 48 * HOUR;
pub const ESCAPE_TIME: i64 = BID_DEADLINE + PAYMENT_WINDOW + 7 * DAY;
pub const DEPOSIT_BPS: u16 = 2000;
pub const DOLLAR: u64 = 1_000_000;
pub const MIN_DEPOSIT: u64 = 10 * DOLLAR;

pub fn usd(dollars: u64) -> u64 {
    dollars * DOLLAR
}

pub struct Bidder {
    pub keypair: Keypair,
    pub token_account: Pubkey,
}

pub struct PlacedBid {
    pub bidder: Bidder,
    pub accounts: BidAccounts,
    pub seed: Pubkey,
}

pub struct World {
    pub svm: LiteSVM,
    pub payer: Keypair,
    pub admin: Keypair,
    pub mint: Pubkey,
    pub treasury_owner: Pubkey,
    pub treasury: Pubkey,
    pub auction_id: Pubkey,
    pub auction: Pubkey,
    pub vault: Pubkey,
}

impl World {
    /// A world with a mint, a treasury, and a created auction with the standard terms.
    pub fn new() -> Self {
        let mut world = Self::without_auction();
        let terms = world.terms();
        world
            .send(client::create_auction(&world.auction_accounts(), &terms), &[&world.admin.insecure_clone()])
            .unwrap();
        world
    }

    pub fn without_auction() -> Self {
        let mut svm = LiteSVM::new().with_sysvars().with_default_programs();
        svm.set_sysvar(&Clock {
            slot: 1,
            epoch_start_timestamp: START,
            epoch: 0,
            leader_schedule_epoch: 0,
            unix_timestamp: START,
        });
        let program_data = include_bytes!("../../../target/deploy/airspace_deposit.so");
        let _ = svm.add_program(PROGRAM_ID, program_data);

        let payer = Keypair::new();
        svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();
        let admin = Keypair::new();
        svm.airdrop(&admin.pubkey(), 10_000_000_000).unwrap();

        let mut world = Self {
            svm,
            payer,
            admin,
            mint: Pubkey::new_unique(),
            treasury_owner: Pubkey::new_unique(),
            treasury: Pubkey::default(),
            auction_id: Pubkey::new_unique(),
            auction: Pubkey::default(),
            vault: Pubkey::default(),
        };
        world.set_mint(world.mint, 6);
        world.treasury = world.set_token_account(&world.treasury_owner.clone(), &world.mint.clone(), 0);
        world.auction = client::auction_address(&world.admin.pubkey(), &world.auction_id);
        world.vault = get_associated_token_address(&world.auction, &world.mint);
        world
    }

    pub fn terms(&self) -> Terms {
        Terms {
            auction_id: self.auction_id,
            deposit_bps: DEPOSIT_BPS,
            min_deposit: MIN_DEPOSIT,
            bid_deadline: BID_DEADLINE,
            payment_window: PAYMENT_WINDOW,
            escape_time: ESCAPE_TIME,
        }
    }

    pub fn auction_accounts(&self) -> Accounts {
        Accounts {
            admin: self.admin.pubkey(),
            auction: self.auction,
            mint: self.mint,
            treasury: self.treasury,
            vault: self.vault,
        }
    }

    pub fn set_mint(&mut self, mint: Pubkey, decimals: u8) {
        let state = Mint {
            mint_authority: COption::Some(self.payer.pubkey()),
            supply: 0,
            decimals,
            is_initialized: true,
            freeze_authority: COption::None,
        };
        let mut data = vec![0u8; Mint::LEN];
        state.pack_into_slice(&mut data);
        self.set_account(mint, data, TOKEN_PROGRAM_ID);
    }

    fn set_account(&mut self, address: Pubkey, data: Vec<u8>, owner: Pubkey) {
        let lamports = self.svm.minimum_balance_for_rent_exemption(data.len());
        self.svm.set_account(address, Account { lamports, data, owner, executable: false, rent_epoch: 0 }).unwrap();
    }

    /// Sets the associated token account of `owner` for `mint` with a balance, and returns it.
    pub fn set_token_account(&mut self, owner: &Pubkey, mint: &Pubkey, amount: u64) -> Pubkey {
        let address = get_associated_token_address(owner, mint);
        self.set_token_account_at(address, owner, mint, amount);
        address
    }

    pub fn set_token_account_at(&mut self, address: Pubkey, owner: &Pubkey, mint: &Pubkey, amount: u64) {
        let state = TokenAccount {
            mint: *mint,
            owner: *owner,
            amount,
            delegate: COption::None,
            state: AccountState::Initialized,
            is_native: COption::None,
            delegated_amount: 0,
            close_authority: COption::None,
        };
        let mut data = vec![0u8; TokenAccount::LEN];
        state.pack_into_slice(&mut data);
        self.set_account(address, data, TOKEN_PROGRAM_ID);
    }

    pub fn balance(&self, token_account: &Pubkey) -> u64 {
        let account = self.svm.get_account(token_account).expect("token account exists");
        u64::from_le_bytes(account.data[64..72].try_into().unwrap())
    }

    pub fn lamports(&self, address: &Pubkey) -> u64 {
        self.svm.get_account(address).map(|a| a.lamports).unwrap_or(0)
    }

    pub fn exists(&self, address: &Pubkey) -> bool {
        self.svm.get_account(address).map(|a| a.lamports > 0).unwrap_or(false)
    }

    pub fn warp(&mut self, unix_timestamp: i64) {
        self.svm.set_sysvar(&Clock {
            slot: 1,
            epoch_start_timestamp: unix_timestamp,
            epoch: 0,
            leader_schedule_epoch: 0,
            unix_timestamp,
        });
        self.svm.expire_blockhash();
    }

    fn transaction(&self, instruction: Instruction, signers: &[&Keypair]) -> Transaction {
        let mut all: Vec<&dyn Signer> = vec![&self.payer];
        all.extend(signers.iter().map(|k| *k as &dyn Signer));
        Transaction::new_signed_with_payer(
            &[instruction],
            Some(&self.payer.pubkey()),
            &all,
            self.svm.latest_blockhash(),
        )
    }

    /// Sends an instruction and returns the data of its inner instructions, which carry the events.
    pub fn send(&mut self, instruction: Instruction, signers: &[&Keypair]) -> Result<Vec<Vec<u8>>, TransactionError> {
        let transaction = self.transaction(instruction, signers);
        match self.svm.send_transaction(transaction) {
            Ok(meta) => {
                Ok(meta.inner_instructions.iter().flatten().map(|inner| inner.instruction.data.clone()).collect())
            }
            Err(failed) => Err(failed.err),
        }
    }

    pub fn send_ok(&mut self, instruction: Instruction, signers: &[&Keypair]) -> Vec<Vec<u8>> {
        self.send(instruction, signers).expect("the transaction should succeed")
    }

    pub fn send_err(&mut self, instruction: Instruction, signers: &[&Keypair]) -> TransactionError {
        self.svm.expire_blockhash();
        self.send(instruction, signers).expect_err("the transaction should fail")
    }

    pub fn new_bidder(&mut self, dollars: u64) -> Bidder {
        let keypair = Keypair::new();
        self.svm.airdrop(&keypair.pubkey(), 1_000_000_000).unwrap();
        let mint = self.mint;
        let token_account = self.set_token_account(&keypair.pubkey(), &mint, usd(dollars));
        Bidder { keypair, token_account }
    }

    pub fn bid_accounts(&self, bidder: &Pubkey, seed: &Pubkey) -> BidAccounts {
        BidAccounts {
            auction: self.auction,
            bid: client::bid_address(&self.auction, bidder, seed),
            vault: self.vault,
            mint: self.mint,
        }
    }

    /// A new bidder with `wallet` dollars places a bid of `bid` dollars, at the start of the auction.
    pub fn place(&mut self, wallet: u64, bid: u64) -> PlacedBid {
        let bidder = self.new_bidder(wallet);
        let seed = Pubkey::new_unique();
        let accounts = self.bid_accounts(&bidder.keypair.pubkey(), &seed);
        let ix = client::place_deposit(&bidder.keypair.pubkey(), &bidder.token_account, &accounts, &seed, usd(bid));
        self.send_ok(ix, &[&bidder.keypair]);
        PlacedBid { bidder, accounts, seed }
    }

    pub fn declare(&mut self, bid: &PlacedBid) {
        self.warp(BID_DEADLINE + 1);
        let ix = client::declare_winner(&self.admin.pubkey(), &bid.accounts);
        self.send_ok(ix, &[&self.admin.insecure_clone()]);
    }

    pub fn pay(&mut self, bid: &PlacedBid) {
        let ix = client::pay_remainder(&bid.bidder.keypair.pubkey(), &bid.bidder.token_account, &bid.accounts);
        self.send_ok(ix, &[&bid.bidder.keypair]);
    }

    pub fn auction_state(&self) -> Auction {
        Auction::from_bytes(&self.svm.get_account(&self.auction).expect("auction exists").data).unwrap()
    }

    pub fn bid_state(&self, bid: &PlacedBid) -> Bid {
        Bid::from_bytes(&self.svm.get_account(&bid.accounts.bid).expect("bid exists").data).unwrap()
    }
}

pub fn assert_deposit_error(error: TransactionError, expected: DepositProgramError) {
    let code = match pinocchio::error::ProgramError::from(expected) {
        pinocchio::error::ProgramError::Custom(code) => code,
        other => panic!("a deposit error maps to a custom program error, got {other:?}"),
    };
    assert_instruction_error(error, InstructionError::Custom(code));
}

pub fn assert_instruction_error(error: TransactionError, expected: InstructionError) {
    match error {
        TransactionError::InstructionError(_, actual) => assert_eq!(actual, expected),
        other => panic!("expected an instruction error, got {other:?}"),
    }
}

/// Finds the fields of the event with this number among the inner instructions of a transaction.
pub fn event_fields(inner: &[Vec<u8>], number: u8) -> Vec<u8> {
    let tag = airspace_deposit::events::EVENT_IX_TAG_LE;
    let found = inner.iter().find(|data| data.len() > tag.len() && data.starts_with(tag) && data[tag.len()] == number);
    found.expect("the instruction emits its event")[tag.len() + 1..].to_vec()
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}
