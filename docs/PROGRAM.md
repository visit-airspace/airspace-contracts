# AirSpace escrow program reference

This is the hand written reference for `programs/airspace-escrow`. The program has no generated IDL. Every instruction starts with a one byte discriminator, then its data fields in little endian order. In the account lists below, w means writable, s means signer, r means read only. The last accounts of every instruction that emits an event are event_authority and escrow_program.

Program address: `A2afr3RxBohFjP5zXhEsJDXU3se43dkd8XT2Bm8o5iKc`.

## Addresses of the accounts

1. escrow: seeds `escrow`, escrow_seed.
2. extensions: seeds `extensions`, escrow.
3. allowed_mint: seeds `allowed_mint`, escrow, mint.
4. receipt: seeds `receipt`, escrow, depositor, mint, receipt_seed.
5. event_authority: seed `event_authority`.
6. vault: the associated token account of the escrow for the mint.

## Instructions

The accounts system_program, event_authority, and escrow_program, and token_program where listed, are the fixed addresses the names say.

0. CreateEscrow. Accounts: payer (w, s), admin (s), escrow_seed (s), escrow (w), system_program, event_authority, escrow_program. Data: bump u8.
1. AddTimelock. Accounts: payer (w, s), admin (s), escrow, extensions (w), system_program, event_authority, escrow_program. Data: extensions_bump u8, lock_duration u64.
2. SetHook. Same accounts as AddTimelock. Data: extensions_bump u8, hook_program Pubkey.
3. Deposit. Accounts: payer (w, s), depositor (s), escrow, allowed_mint, receipt_seed (s), receipt (w), vault (w), depositor_token_account (w), mint, token_program, system_program, event_authority, escrow_program, extensions. Data: bump u8, amount u64. Remaining accounts: the hook program and its extra accounts, if a hook is set. Refused with DepositDeadlinePassed after the deposit deadline of the settlement terms.
4. UpdateAdmin. Accounts: admin (s), new_admin (s), escrow (w), event_authority, escrow_program. No data.
5. Withdraw. Accounts: rent_recipient (w), withdrawer (s), escrow, extensions, receipt (w), vault (w), withdrawer_token_account (w), mint, token_program, system_program, event_authority, escrow_program. No data. Remaining accounts: the arbiter signer first if an arbiter is set, then the hook program and its extra accounts. Refused with EscapeTimeNotReached before the escape time of the settlement terms.
6. AllowMint. Accounts: payer (w, s), admin (s), escrow, escrow_extensions, mint, allowed_mint (w), vault (w), token_program, associated_token_program, system_program, event_authority, escrow_program. Data: bump u8.
7. BlockMint. Accounts: admin (s), rent_recipient (w), escrow, mint, allowed_mint (w), token_program, event_authority, escrow_program. No data.
8. BlockTokenExtension. Same accounts as AddTimelock. Data: extensions_bump u8, blocked_extension u16.
9. SetArbiter (upstream). Accounts: payer (w, s), admin (s), arbiter (s), escrow, extensions (w), system_program, event_authority, escrow_program. Data: extensions_bump u8.
10. RemoveExtension. Same accounts as AddTimelock. Data: extensions_bump u8, extension_type u16. Refuses extension type 4 with SettlementTermsPermanent.
11. UnblockTokenExtension. Same accounts as AddTimelock. Data: extensions_bump u8, blocked_extension u16.
12. SetImmutable. Accounts: admin (s), escrow (w), event_authority, escrow_program. No data.
13. SetSettlement (AirSpace). Same accounts as AddTimelock. Data: extensions_bump u8, beneficiary Pubkey, deposit_deadline i64, escape_time i64, 49 bytes with the discriminator excluded. Admin only. Refuses with InvalidSettlementTerms when escape_time is less than 7 days after deposit_deadline, and with SettlementAlreadySet when the terms exist.
14. Refund (AirSpace). Accounts: rent_recipient (w), admin (s), escrow, extensions, receipt (w), vault (w), recipient_token_account (w), mint, token_program, system_program, event_authority, escrow_program. No data. Remaining accounts: the hook program and its extra accounts, if a hook is set. Admin only. rent_recipient must be the receipt's depositor, recipient_token_account must be the depositor's associated token account for the mint. Pays the whole receipt amount from the vault and closes the receipt.
15. Settle (AirSpace). Same accounts and rules as Refund, except that recipient_token_account must be the associated token account of the settlement beneficiary.
228. EmitEvent. Internal self call that carries an event. Only the event_authority can sign it.

Refund and Settle need settlement terms, so on an escrow without them they fail with SettlementNotSet. Both run the hook at PreWithdraw and PostWithdraw.

## Extensions

The extensions account holds a header and a list of extensions, each with a type and its data. Types: 0 Timelock, 1 Hook, 2 BlockedTokenExtensions, 3 Arbiter, 4 Settlement.

Settlement data is 48 bytes: beneficiary (32 bytes), deposit_deadline i64, escape_time i64.

## Events

An event is the program calling itself through EmitEvent. The data is the event tag, then one byte event number, then the fields. Numbers: 0 CreatesEscrow, 1 TimelockAdded, 2 HookSet, 3 Deposit, 4 AdminUpdate, 5 Withdraw, 6 AllowMint, 7 BlockMint, 8 TokenExtensionBlocked, 9 ArbiterSet, 10 ExtensionRemoved, 11 TokenExtensionUnblocked, 12 SetImmutable, 13 SettlementSet, 14 Refund, 15 Settle.

1. SettlementSet fields: escrow, beneficiary, deposit_deadline i64, escape_time i64.
2. Refund and Settle fields: escrow, receipt, depositor, recipient (the token account that was paid), mint, amount u64.

## Errors

Custom program errors, numbered from 0: 0 InvalidEscrowId, 1 InvalidAdmin, 2 InvalidEventAuthority, 3 TimelockNotExpired, 4 HookRejected, 5 InvalidWithdrawer, 6 InvalidReceiptEscrow, 7 HookProgramMismatch, 8 MintNotAllowed, 9 PermanentDelegateNotAllowed, 10 NonTransferableNotAllowed, 11 PausableNotAllowed, 12 TokenExtensionAlreadyBlocked, 13 ZeroDepositAmount, 14 InvalidArbiter, 15 TokenExtensionNotBlocked, 16 EscrowImmutable.

AirSpace additions: 17 SettlementAlreadySet, 18 InvalidSettlementTerms, 19 DepositDeadlinePassed, 20 EscapeTimeNotReached, 21 SettlementNotSet, 22 SettlementTermsPermanent, 23 InvalidPayoutRecipient, 24 InvalidRentRecipient.
