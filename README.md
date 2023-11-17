# AssetDesk

This repository contains the smart contacts for an implementation of the AssetDesk. AssetDesk is a decentralized non-custodial liquidity protocol where users can participate as depositors or borrowers. Depositors provide liquidity to the market to earn a passive income, while borrowers are able to borrow in an overcollateralized (perpetually) or undercollateralized (one-block liquidity) fashion.

## Demo available

Front-end application working with the deployed testnet smart contract is available:
* [https://assetdesk.xyz/](https://assetdesk.xyz/)

## Files description

|                                    |                                                                          |
|------------------------------------|--------------------------------------------------------------------------|
| [src](./contracts/lending)         | contains the lending contract files.                                     |
| [src](./contracts/vault_contracts) | contains the vault contract files.                                       |
| [scripts](./scripts)               | contains deployment and js interaction scripts for the testnet contract. |
| [token](./token)                   | contains a standard token wasm file for testing.                         |

## Core functionality

1) **Deposit**: Users can safely store their assets into the AssetDesk protocol. Once deposited, these assets immediately start accruing interest, allowing users to grow their holdings over time.
2) **Redeem**: assetDesk allows for easy and convenient withdrawal of assets, including any accrued interest, ensuring users have constant access to their funds.
3) **Borrow**: Users can leverage their deposited assets as collateral to secure loans. This provides an efficient method to access additional funds without needing to liquidate existing holdings.
4) **Repay**: assetDesk facilitates seamless repayment of borrowed assets. On completion of repayment, the accumulated interest is settled, reducing potential risk against the user's collateral.

## Documentation

To learn more about the AssetDesk, visit the docs:
* [AssetDesk Docs](https://assetdesk.gitbook.io/)

## Audits

No audits have been conducted for the protocol at this time. Results will be included here at the conclusion of an audit.

## Community Links

A set of links for various things in the community. Please submit a pull request if you would like a link included.

* [AssetDesk on Stellar Discord](https://discord.com/channels/897514728459468821/1082054199187083264/threads/1167387469960990750)

## Developers

Build & Test contracts using command:


<code>cd contracts && cargo build --target wasm32-unknown-unknown --release  && cargo test</code>
