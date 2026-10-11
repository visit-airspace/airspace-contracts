//! Hand built instruction builders for the AirSpace escrow program.
//!
//! The upstream tests build their instructions through a client that Codama
//! generated from the program. The fork removes Codama, so this module writes
//! the same instructions by hand: the account order and the data layout of each
//! instruction are the ones that `docs/PROGRAM.md` documents. A builder keeps the
//! method names the tests already use, one setter for each account and each data
//! field, and `instruction()` fails loudly when an account or a field was never
//! set.

use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};

use crate::utils::{find_event_authority_pda, ESCROW_PROGRAM_ID};

pub const SYSTEM_PROGRAM_ID: Pubkey = solana_system_interface::program::ID;
pub const TOKEN_PROGRAM_ID: Pubkey = spl_token_interface::ID;
pub const ASSOCIATED_TOKEN_PROGRAM_ID: Pubkey = spl_associated_token_account::ID;

pub fn event_authority() -> Pubkey {
    find_event_authority_pda().0
}

/// Which flags an account carries in an instruction.
#[derive(Clone, Copy)]
pub enum Access {
    SignerWritable,
    Signer,
    Writable,
    Readonly,
}

impl Access {
    pub fn meta(self, key: Pubkey) -> AccountMeta {
        match self {
            Access::SignerWritable => AccountMeta::new(key, true),
            Access::Signer => AccountMeta::new_readonly(key, true),
            Access::Writable => AccountMeta::new(key, false),
            Access::Readonly => AccountMeta::new_readonly(key, false),
        }
    }
}

/// A value that one data field writes in little endian order.
pub trait DataField: Copy {
    fn write(self, out: &mut Vec<u8>);
}

impl DataField for u8 {
    fn write(self, out: &mut Vec<u8>) {
        out.push(self);
    }
}

impl DataField for u16 {
    fn write(self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.to_le_bytes());
    }
}

impl DataField for u32 {
    fn write(self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.to_le_bytes());
    }
}

impl DataField for u64 {
    fn write(self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.to_le_bytes());
    }
}

impl DataField for i64 {
    fn write(self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.to_le_bytes());
    }
}

impl DataField for Pubkey {
    fn write(self, out: &mut Vec<u8>) {
        out.extend_from_slice(self.as_ref());
    }
}

/// Declares one instruction builder.
///
/// Each account is `name: Access` or `name: Access = default`, in the order the
/// program reads them. Each data field is `name: type`, in the order the program
/// reads them, after the one byte discriminator.
macro_rules! instruction_builder {
    (
        $builder:ident, $discriminator:expr,
        accounts: [ $( $account:ident : $access:ident $( = $default:expr )? ),* $(,)? ],
        data: [ $( $field:ident : $field_type:ty ),* $(,)? ]
    ) => {
        #[derive(Default)]
        pub struct $builder {
            $( $account: Option<Pubkey>, )*
            $( $field: Option<$field_type>, )*
            remaining: Vec<AccountMeta>,
        }

        #[allow(dead_code)]
        impl $builder {
            pub fn new() -> Self {
                Self::default()
            }

            $(
                pub fn $account(&mut self, key: Pubkey) -> &mut Self {
                    self.$account = Some(key);
                    self
                }
            )*

            $(
                pub fn $field(&mut self, value: $field_type) -> &mut Self {
                    self.$field = Some(value);
                    self
                }
            )*

            pub fn add_remaining_account(&mut self, account: AccountMeta) -> &mut Self {
                self.remaining.push(account);
                self
            }

            pub fn add_remaining_accounts(&mut self, accounts: &[AccountMeta]) -> &mut Self {
                self.remaining.extend_from_slice(accounts);
                self
            }

            pub fn instruction(&self) -> Instruction {
                let mut accounts: Vec<AccountMeta> = Vec::new();
                $(
                    let key = self.$account
                        $( .or(Some($default)) )?
                        .unwrap_or_else(|| panic!(concat!(stringify!($builder), " needs the account ", stringify!($account))));
                    accounts.push(Access::$access.meta(key));
                )*
                accounts.extend_from_slice(&self.remaining);

                #[allow(unused_mut)]
                let mut data: Vec<u8> = vec![$discriminator];
                $(
                    DataField::write(
                        self.$field.unwrap_or_else(|| panic!(concat!(stringify!($builder), " needs the data field ", stringify!($field)))),
                        &mut data,
                    );
                )*

                Instruction { program_id: ESCROW_PROGRAM_ID, accounts, data }
            }
        }
    };
}

instruction_builder!(
    CreatesEscrowBuilder, 0,
    accounts: [
        payer: SignerWritable,
        admin: Signer,
        escrow_seed: Signer,
        escrow: Writable,
        system_program: Readonly = SYSTEM_PROGRAM_ID,
        event_authority: Readonly = event_authority(),
        escrow_program: Readonly = ESCROW_PROGRAM_ID,
    ],
    data: [bump: u8]
);

instruction_builder!(
    AddTimelockBuilder, 1,
    accounts: [
        payer: SignerWritable,
        admin: Signer,
        escrow: Readonly,
        extensions: Writable,
        system_program: Readonly = SYSTEM_PROGRAM_ID,
        event_authority: Readonly = event_authority(),
        escrow_program: Readonly = ESCROW_PROGRAM_ID,
    ],
    data: [extensions_bump: u8, lock_duration: u64]
);

instruction_builder!(
    SetHookBuilder, 2,
    accounts: [
        payer: SignerWritable,
        admin: Signer,
        escrow: Readonly,
        extensions: Writable,
        system_program: Readonly = SYSTEM_PROGRAM_ID,
        event_authority: Readonly = event_authority(),
        escrow_program: Readonly = ESCROW_PROGRAM_ID,
    ],
    data: [extensions_bump: u8, hook_program: Pubkey]
);

instruction_builder!(
    DepositBuilder, 3,
    accounts: [
        payer: SignerWritable,
        depositor: Signer,
        escrow: Readonly,
        allowed_mint: Readonly,
        receipt_seed: Signer,
        receipt: Writable,
        vault: Writable,
        depositor_token_account: Writable,
        mint: Readonly,
        token_program: Readonly = TOKEN_PROGRAM_ID,
        system_program: Readonly = SYSTEM_PROGRAM_ID,
        event_authority: Readonly = event_authority(),
        escrow_program: Readonly = ESCROW_PROGRAM_ID,
        extensions: Readonly,
    ],
    data: [bump: u8, amount: u64]
);

instruction_builder!(
    UpdateAdminBuilder, 4,
    accounts: [
        admin: Signer,
        new_admin: Signer,
        escrow: Writable,
        event_authority: Readonly = event_authority(),
        escrow_program: Readonly = ESCROW_PROGRAM_ID,
    ],
    data: []
);

instruction_builder!(
    WithdrawBuilder, 5,
    accounts: [
        rent_recipient: Writable,
        withdrawer: Signer,
        escrow: Readonly,
        extensions: Readonly,
        receipt: Writable,
        vault: Writable,
        withdrawer_token_account: Writable,
        mint: Readonly,
        token_program: Readonly = TOKEN_PROGRAM_ID,
        system_program: Readonly = SYSTEM_PROGRAM_ID,
        event_authority: Readonly = event_authority(),
        escrow_program: Readonly = ESCROW_PROGRAM_ID,
    ],
    data: []
);

instruction_builder!(
    AllowMintBuilder, 6,
    accounts: [
        payer: SignerWritable,
        admin: Signer,
        escrow: Readonly,
        escrow_extensions: Readonly,
        mint: Readonly,
        allowed_mint: Writable,
        vault: Writable,
        token_program: Readonly = TOKEN_PROGRAM_ID,
        associated_token_program: Readonly = ASSOCIATED_TOKEN_PROGRAM_ID,
        system_program: Readonly = SYSTEM_PROGRAM_ID,
        event_authority: Readonly = event_authority(),
        escrow_program: Readonly = ESCROW_PROGRAM_ID,
    ],
    data: [bump: u8]
);

instruction_builder!(
    BlockMintBuilder, 7,
    accounts: [
        admin: Signer,
        rent_recipient: Writable,
        escrow: Readonly,
        mint: Readonly,
        allowed_mint: Writable,
        token_program: Readonly = TOKEN_PROGRAM_ID,
        event_authority: Readonly = event_authority(),
        escrow_program: Readonly = ESCROW_PROGRAM_ID,
    ],
    data: []
);

instruction_builder!(
    BlockTokenExtensionBuilder, 8,
    accounts: [
        payer: SignerWritable,
        admin: Signer,
        escrow: Readonly,
        extensions: Writable,
        system_program: Readonly = SYSTEM_PROGRAM_ID,
        event_authority: Readonly = event_authority(),
        escrow_program: Readonly = ESCROW_PROGRAM_ID,
    ],
    data: [extensions_bump: u8, blocked_extension: u16]
);

instruction_builder!(
    SetArbiterBuilder, 9,
    accounts: [
        payer: SignerWritable,
        admin: Signer,
        arbiter: Signer,
        escrow: Readonly,
        extensions: Writable,
        system_program: Readonly = SYSTEM_PROGRAM_ID,
        event_authority: Readonly = event_authority(),
        escrow_program: Readonly = ESCROW_PROGRAM_ID,
    ],
    data: [extensions_bump: u8]
);

instruction_builder!(
    RemoveExtensionBuilder, 10,
    accounts: [
        payer: SignerWritable,
        admin: Signer,
        escrow: Readonly,
        extensions: Writable,
        system_program: Readonly = SYSTEM_PROGRAM_ID,
        event_authority: Readonly = event_authority(),
        escrow_program: Readonly = ESCROW_PROGRAM_ID,
    ],
    data: [extensions_bump: u8, extension_type: u16]
);

instruction_builder!(
    UnblockTokenExtensionBuilder, 11,
    accounts: [
        payer: SignerWritable,
        admin: Signer,
        escrow: Readonly,
        extensions: Writable,
        system_program: Readonly = SYSTEM_PROGRAM_ID,
        event_authority: Readonly = event_authority(),
        escrow_program: Readonly = ESCROW_PROGRAM_ID,
    ],
    data: [extensions_bump: u8, blocked_extension: u16]
);

instruction_builder!(
    SetImmutableBuilder, 12,
    accounts: [
        admin: Signer,
        escrow: Writable,
        event_authority: Readonly = event_authority(),
        escrow_program: Readonly = ESCROW_PROGRAM_ID,
    ],
    data: []
);

instruction_builder!(
    SetSettlementBuilder, 13,
    accounts: [
        payer: SignerWritable,
        admin: Signer,
        escrow: Readonly,
        extensions: Writable,
        system_program: Readonly = SYSTEM_PROGRAM_ID,
        event_authority: Readonly = event_authority(),
        escrow_program: Readonly = ESCROW_PROGRAM_ID,
    ],
    data: [extensions_bump: u8, beneficiary: Pubkey, deposit_deadline: i64, escape_time: i64]
);

instruction_builder!(
    RefundBuilder, 14,
    accounts: [
        rent_recipient: Writable,
        admin: Signer,
        escrow: Readonly,
        extensions: Readonly,
        receipt: Writable,
        vault: Writable,
        recipient_token_account: Writable,
        mint: Readonly,
        token_program: Readonly = TOKEN_PROGRAM_ID,
        system_program: Readonly = SYSTEM_PROGRAM_ID,
        event_authority: Readonly = event_authority(),
        escrow_program: Readonly = ESCROW_PROGRAM_ID,
    ],
    data: []
);

instruction_builder!(
    SettleBuilder, 15,
    accounts: [
        rent_recipient: Writable,
        admin: Signer,
        escrow: Readonly,
        extensions: Readonly,
        receipt: Writable,
        vault: Writable,
        recipient_token_account: Writable,
        mint: Readonly,
        token_program: Readonly = TOKEN_PROGRAM_ID,
        system_program: Readonly = SYSTEM_PROGRAM_ID,
        event_authority: Readonly = event_authority(),
        escrow_program: Readonly = ESCROW_PROGRAM_ID,
    ],
    data: []
);
