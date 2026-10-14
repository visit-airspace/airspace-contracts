# AirSpace deposit program reference

This is the hand written reference for `programs/airspace-deposit`. The program is new code, not a fork, and it is unaudited. It has no generated IDL. Every instruction starts with a one byte discriminator, then its data fields in little endian order. In the account lists, w means writable, s means signer, and the last two accounts of every instruction that works on an auction are event_authority and program (the deposit program itself).

Program address: `FSoKNTwhgG36NAxu5Axq84yvFZT1ASVz4muVxETwwjKk`.

## How an auction works

1. The admin creates the auction with its terms. The terms never change.
2. Bidders place deposits until the bid deadline. The program computes the deposit from the bid: the deposit share of the bid amount rounded up to a whole dollar, and never less than the minimum deposit. A bid must be a whole number of dollars and at least the minimum deposit.
3. The admin refunds the bids that lost, and after the bid deadline declares one bid the winner. That starts the winner's payment window, and a declaration is only accepted while the window ends before the escape time.
4. The winner pays exactly the bid amount minus the deposit before the remainder deadline, and then anyone may settle the sale, which moves the whole bid to the treasury.
5. If the winner does not pay, anyone may forfeit the bid after the remainder deadline, which moves only the deposit to the treasury.
6. From the escape time on, a bidder whose bid is still in state deposited takes the deposit back without the admin. A winner or paid bid can only end through Settle or Forfeit.

The program accepts one mint per auction, which must have 6 decimals and no token extensions, so one dollar is 1,000,000 base units. The token program may be SPL Token or Token-2022, and every transfer uses `transfer_checked`. The program never pulls money from a wallet: every token in the vault came from a deposit or a remainder that its bidder signed. The bidder pays the rent of a bid and gets it back when the bid closes, and the bidder pays the transaction fees of the instructions it signs.

## Accounts

1. Auction, seeds `auction`, admin, auction_id (32 bytes). 229 bytes: discriminator 1, version 1, bump u8, auction_id, admin, mint, treasury (token account), vault (associated token account of the auction), deposit_bps u16, min_deposit u64, bid_deadline i64, payment_window i64, escape_time i64, winner_bid (zero address until declared).
2. Bid, seeds `bid`, auction, bidder, bid_seed (32 bytes). 132 bytes: discriminator 2, version 1, bump u8, state u8 (0 deposited, 1 winner, 2 paid), auction, bidder, bid_seed, amount u64, deposit u64, remainder_paid u64, remainder_deadline i64.
3. event_authority, seed `event_authority`.

## Instructions

0. CreateAuction. Accounts: admin (w, s, pays the rent of the auction), auction (w), mint, treasury, vault (w), token_program, associated_token_program, system_program, event_authority, program. Data, 66 bytes (67 including the discriminator): auction_id (32), deposit_bps u16, min_deposit u64, bid_deadline i64, payment_window i64, escape_time i64. Refuses a deposit share outside 1000 to 5000 or not a multiple of 100, a minimum deposit that is zero or not whole dollars, a bid deadline that is not in the future, a payment window that is not positive, and an escape time less than 7 days after the bid deadline plus the payment window. Also refuses a treasury that is not the associated token account of its owner for the mint, or that carries token extensions besides the ImmutableOwner of a Token-2022 associated token account. A paid bid can only end in Settle, which pays the treasury, so the treasury must stay payable: anyone can recreate it if it is closed. The treasury owner must not later enable required memos on it, and the mint's freeze authority can still freeze it. Also refuses a treasury that is not the associated token account of its owner for the mint, or that carries token extensions besides the ImmutableOwner of a Token-2022 associated token account. A paid bid can only end in Settle, which pays the treasury, so the treasury must stay payable: anyone can recreate it if it is closed. The treasury owner must not later enable required memos on it, and the mint's freeze authority can still freeze it. AirSpace auctions use a minimum deposit of 10,000,000 and a payment window of 48 hours.
1. PlaceDeposit. Accounts: bidder (w, s), auction, bid (w), vault (w), bidder_token_account (w), mint, token_program, system_program, event_authority, program. Data, 40 bytes: bid_seed (32), amount u64. Refused after the bid deadline.
2. RefundDeposit. Accounts: admin (s), bidder (w, receives the rent), auction, bid (w), vault (w), bidder_token_account (w), mint, token_program, event_authority, program. No data. Admin only. Pays the bidder's associated token account and nothing else, and only for a bid in state deposited.
3. DeclareWinner. Accounts: admin (s), auction (w), bid (w), event_authority, program. No data. Admin only, after the bid deadline, once per auction, for a bid in state deposited.
4. PayRemainder. Accounts: bidder (s), auction, bid (w), vault (w), bidder_token_account (w), mint, token_program, event_authority, program. No data. Only the winning bidder, only for a bid in state winner, until the remainder deadline. The amount is computed by the program.
5. Settle. Accounts: bidder (w, receives the rent), auction, bid (w), vault (w), treasury (w), mint, token_program, event_authority, program. No data. No signer besides the fee payer. Only for a bid in state paid, and only to the auction's treasury.
6. Forfeit. Same accounts as Settle. No data. No signer besides the fee payer. Only for a bid in state winner, after its remainder deadline, and moves only the deposit.
7. Escape. Accounts: bidder (w, s), auction, bid (w), vault (w), bidder_token_account (w), mint, token_program, event_authority, program. No data. Only the bidder, only for a bid in state deposited, and only from the escape time on.
228. EmitEvent. Internal self call that carries an event. Only the event_authority can sign it.

## Events

An event is the program calling itself through EmitEvent. The data is the 8 byte event tag, one byte event number, then the fields.

0. AuctionCreated: auction, admin, mint, treasury, deposit_bps u16, min_deposit u64, bid_deadline i64, payment_window i64, escape_time i64.
1 to 7, DepositPlaced, DepositRefunded, WinnerDeclared, RemainderPaid, BidSettled, BidForfeited, BidEscaped: auction, bid, bidder, recipient, amount u64. The recipient is the token account that received money, the vault for a deposit or a remainder, and the zero address for WinnerDeclared, where no money moves and the amount is the bid amount.

## Errors

Custom program errors, numbered from 0: 0 InvalidEventAuthority, 1 InvalidAdmin, 2 InvalidBidder, 3 InvalidDepositShare, 4 InvalidMinimumDeposit, 5 InvalidAuctionTerms, 6 BidDeadlinePassed, 7 BidDeadlineNotReached, 8 InvalidBidAmount, 9 InvalidMint, 10 MintHasExtensions, 11 InvalidBidState, 12 WinnerAlreadyDeclared, 13 PaymentWindowPassesEscape, 14 RemainderDeadlinePassed, 15 RemainderDeadlineNotPassed, 16 EscapeTimeNotReached, 17 InvalidVault, 18 InvalidTreasury, 19 InvalidRecipient, 20 InvalidSeeds, 21 ArithmeticOverflow, 22 InvalidBidAuction.
