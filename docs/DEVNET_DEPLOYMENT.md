# Devnet deployment

Both AirSpace programs were deployed to Solana devnet on October 13, 2026, from source commit `3febdd91`. Both executable program accounts were verified at finalized commitment. Testnet and mainnet deployments were not performed.

| Program | Program address | ProgramData address | Deployment slot |
| :--- | :--- | :--- | ---: |
| Escrow | `A2afr3RxBohFjP5zXhEsJDXU3se43dkd8XT2Bm8o5iKc` | `12wB3HbSiY8kDTnhpaPwo61muQBB4W9i1hVeCFR6apMx` | 510452757 |
| Deposit | `FSoKNTwhgG36NAxu5Axq84yvFZT1ASVz4muVxETwwjKk` | `5KXcBsJxaRmLQZHSVTfY1m9x7EBdC27DKwU7yCpyq4KN` | 510452834 |

1. [Escrow program on Solana Explorer](https://explorer.solana.com/address/A2afr3RxBohFjP5zXhEsJDXU3se43dkd8XT2Bm8o5iKc?cluster=devnet).
2. [Deposit program on Solana Explorer](https://explorer.solana.com/address/FSoKNTwhgG36NAxu5Axq84yvFZT1ASVz4muVxETwwjKk?cluster=devnet).
3. [Escrow deployment transaction](https://explorer.solana.com/tx/4GgJ6Yxee17iJMvSG82bpd9vp7u9VyicLG1c8UDjVAzfnvChFd26HbdWhQy4YnCAfWTgYFJ3vqEaCgj8hFmbiYmo?cluster=devnet).
4. [Deposit deployment transaction](https://explorer.solana.com/tx/j33arW81qtwvdvDJaqGbDJqadJjZLysiQUGbMraRFWPxZAw8AyckbBuKqDRYCki9csqVEgSpCefD188wL3sPgE7?cluster=devnet).

## Authority and funding

The fee payer and retained upgrade authority for both programs is `47gEvFMdkLw2NWMFPRZA9vdx8Hp8yfHE8F61rdVX5K76`. Both programs remain upgradeable. All deployment commands explicitly selected devnet.

Deployment used 1.417774800 devnet SOL, including program rent and transaction fees. Both temporary deployment buffer accounts were absent after completion.

## Verification

Both programs were rebuilt with `cargo build-sbf`, and `cargo test --quiet` passed 569 active tests, with two ignored documentation tests. The deployed programs were downloaded with `solana program dump` and matched the local build artifacts byte for byte with `cmp`.

| Binary | Bytes | SHA256 of local and downloaded binary |
| :--- | ---: | :--- |
| `airspace_escrow.so` | 201816 | `9b1500aa6a5babb235d3bdeca08e3ffdf98d13a98b8ab719dc2537b1ec2bb315` |
| `airspace_deposit.so` | 76320 | `97ccc78885def1b9dfdc998d4480d8efcf835df6b7fadcf95e732a0eefa84ee0` |

`solana program show` verified both ProgramData addresses, retained upgrade authority, and deployment slots. A finalized `getMultipleAccounts` query verified both program accounts were executable and owned by `BPFLoaderUpgradeab1e11111111111111111111111`.

## Deployment transport and future updates

The initial upload through public RPC encountered HTTP 429 rate limits. The upload was resumed using the same buffer keypair with `--use-tpu-client --use-quic`, and both deployments completed. Signing keys and local verification artifacts are excluded from this public repository.

Future updates require the retained upgrade authority's signature. Select devnet explicitly, preserve the existing program IDs, and verify the downloaded binary after updating. The initial maximum program lengths equal the binary sizes listed above; a larger future build may require extending ProgramData and paying additional devnet rent.

This deployment publishes the two program binaries. It does not initialize auctions, mint a test token, or establish an end to end auction flow with the AirSpace application.
