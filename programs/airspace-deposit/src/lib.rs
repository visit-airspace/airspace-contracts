//! # AirSpace Deposit Program
//!
//! A Solana program for deposit then remainder auctions. A bidder deposits a share of
//! the bid, the admin declares one winner, the winner pays the remainder inside a
//! payment window, and the sale settles to the treasury. A winner who does not pay
//! forfeits the deposit, and every other deposit goes back to its bidder.
//!
//! ## Architecture
//! Built with Pinocchio (no_std). New code, not a fork: the structure follows the
//! AirSpace escrow program so that a reader of one can read the other.
#![no_std]

extern crate alloc;

use pinocchio::address::declare_id;

pub mod errors;
pub mod events;
pub mod instructions;
pub mod state;
pub mod utils;

#[cfg(not(feature = "no-entrypoint"))]
pub mod entrypoint;

declare_id!("FSoKNTwhgG36NAxu5Axq84yvFZT1ASVz4muVxETwwjKk");

#[cfg(not(feature = "no-entrypoint"))]
use solana_security_txt::security_txt;

#[cfg(not(feature = "no-entrypoint"))]
security_txt! {
    name: "AirSpace Deposit Program",
    project_url: "https://github.com/visit-airspace/airspace-contracts",
    contacts: "email:hello@visitairspace.xyz",
    policy: "https://github.com/visit-airspace/airspace-contracts/blob/main/SECURITY.md",
    source_code: "https://github.com/visit-airspace/airspace-contracts"
}
