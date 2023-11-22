#![cfg(test)]

extern crate std;

use crate::contract::{MarginPositionsContract, MarginPositionsContractClient};
use crate::storage::*;
use std::println;

use soroban_sdk::arbitrary::std::dbg;
use soroban_sdk::testutils::{Address as _, Events, Ledger, LedgerInfo};
use soroban_sdk::token::Interface;
use soroban_sdk::{symbol_short, token, Address, Env, IntoVal, String, Symbol};
use token::Client;

mod token_contract {
    soroban_sdk::contractimport!(file = "./token/soroban_token_contract.optimized.wasm");
}

mod vault_contract {
    soroban_sdk::contractimport!(
        file = "../../target/wasm32-unknown-unknown/release/vault_contract.wasm"
    );
}

fn create_vault_contract<'a>(
    env: &Env,
    admin: &Address,
    lending_contract_address: &Address,
    margin_contract_address: &Address,
) -> (Address, vault_contract::Client<'a>) {
    let vault_contract_address = &env.register_contract_wasm(None, vault_contract::WASM);
    let vault_contract_client = vault_contract::Client::new(env, &vault_contract_address);
    vault_contract_client.initialize(lending_contract_address, margin_contract_address, admin);
    (vault_contract_address.clone(), vault_contract_client)
}

fn create_custom_token<'a>(
    env: &Env,
    admin: &Address,
    name: &str,
    symbol: &str,
    decimals: &u32,
) -> token_contract::Client<'a> {
    let token_id = &env.register_contract_wasm(None, token_contract::WASM);
    let token = token_contract::Client::new(env, &token_id);
    token.initialize(
        admin,
        decimals,
        &String::from_slice(&env, name),
        &String::from_slice(&env, symbol),
    );
    token
}

pub fn success_deposit_of_diff_token_with_prices() -> (
    MarginPositionsContractClient<'static>,
    token_contract::Client<'static>,
    Address,
    Address,
    Address,
) {
    const TOKENS_DECIMALS: u32 = 18;

    const INIT_USER_BALANCE: u128 = 1000 * 10u128.pow(TOKENS_DECIMALS);
    const INIT_LIQUIDATOR_BALANCE_ETH: u128 = 1_000_000 * 10u128.pow(TOKENS_DECIMALS); // 1M ETH

    const CONTRACT_RESERVES_ETH: u128 = 1000 * 10u128.pow(TOKENS_DECIMALS);
    const CONTRACT_RESERVES_XLM: u128 = 1000 * 10u128.pow(TOKENS_DECIMALS);

    const FIRST_DEPOSIT_AMOUNT_ETH: u128 = 200 * 10u128.pow(TOKENS_DECIMALS);
    const SECOND_DEPOSIT_AMOUNT_ETH: u128 = 300 * 10u128.pow(TOKENS_DECIMALS);

    const PERCENT_DECIMALS: u32 = 5;

    const PRICE_DECIMALS: u32 = 8;
    const PRICE_ETH: u128 = 2000 * 10u128.pow(PRICE_DECIMALS);
    const PRICE_XLM: u128 = 10 * 10u128.pow(PRICE_DECIMALS);

    let env = Env::default();
    env.mock_all_auths();

    env.budget().reset_unlimited();
    let margin_contract_address = env.register_contract(None, MarginPositionsContract);
    let margin_contract_client = MarginPositionsContractClient::new(&env, &margin_contract_address);
    let admin = Address::random(&env);
    let user = Address::random(&env);
    let liquidator = Address::random(&env);

    let (vault_contract_address, vault_contract_client) = create_vault_contract(
        &env,
        &admin,
        &margin_contract_address,
        &margin_contract_address,
    );

    margin_contract_client.initialize(&admin, &liquidator, &vault_contract_address);

    let vault_contract_obtained_from_margin: Address = margin_contract_client.get_vault_contract();
    let margin_contract_obtained_from_vault: Address = vault_contract_client.get_lending_contract();

    assert_eq!(vault_contract_obtained_from_margin, vault_contract_address);
    assert_eq!(margin_contract_obtained_from_vault, margin_contract_address);

    let token_xlm = create_custom_token(&env, &admin, "Xlm", "xlm", &TOKENS_DECIMALS);
    let token_eth = create_custom_token(&env, &admin, "Eth", "eth", &TOKENS_DECIMALS);

    token_xlm.mint(&user, &i128::try_from(INIT_USER_BALANCE).unwrap());
    token_eth.mint(&user, &i128::try_from(INIT_USER_BALANCE).unwrap());

    token_xlm.mint(
        &admin,
        &i128::try_from(CONTRACT_RESERVES_XLM * 100).unwrap(),
    );
    token_eth.mint(&admin, &i128::try_from(CONTRACT_RESERVES_ETH).unwrap());

    token_eth.mint(
        &liquidator,
        &i128::try_from(INIT_LIQUIDATOR_BALANCE_ETH).unwrap(),
    );

    margin_contract_client.add_markets(
        &symbol_short!("xlm"),
        &token_xlm.address,
        &symbol_short!("Xlm"),
        &TOKENS_DECIMALS,
    );

    margin_contract_client.add_markets(
        &symbol_short!("eth"),
        &token_eth.address,
        &symbol_short!("Eth"),
        &TOKENS_DECIMALS,
    );

    // Funding vault contract
    token_xlm.transfer(
        &admin,
        &vault_contract_address,
        &i128::try_from(CONTRACT_RESERVES_XLM).unwrap(),
    );
    token_eth.transfer(
        &admin,
        &vault_contract_address,
        &i128::try_from(CONTRACT_RESERVES_ETH).unwrap(),
    );

    margin_contract_client.update_price(&symbol_short!("xlm"), &PRICE_XLM);
    margin_contract_client.update_price(&symbol_short!("eth"), &PRICE_ETH);

    let get_price_xlm: u128 = margin_contract_client.get_price(&symbol_short!("xlm"));
    let get_price_eth: u128 = margin_contract_client.get_price(&symbol_short!("eth"));

    assert_eq!(get_price_xlm, 1000000000); // 10$
    assert_eq!(get_price_eth, 200000000000); // 2000$
    margin_contract_client.deposit(&user, &symbol_short!("eth"), &FIRST_DEPOSIT_AMOUNT_ETH);

    let mut user_deposited_balance: u128 =
        margin_contract_client.get_deposit(&user, &symbol_short!("eth"));

    assert_eq!(user_deposited_balance, FIRST_DEPOSIT_AMOUNT_ETH);
    assert_eq!(
        token_eth.balance(&user),
        (INIT_USER_BALANCE - FIRST_DEPOSIT_AMOUNT_ETH) as i128
    );
    assert_eq!(
        token_eth.balance(&vault_contract_address),
        (CONTRACT_RESERVES_ETH + FIRST_DEPOSIT_AMOUNT_ETH) as i128
    );

    margin_contract_client.deposit(&user, &symbol_short!("eth"), &SECOND_DEPOSIT_AMOUNT_ETH);

    user_deposited_balance = margin_contract_client.get_deposit(&user, &symbol_short!("eth"));

    assert_eq!(
        user_deposited_balance,
        FIRST_DEPOSIT_AMOUNT_ETH + SECOND_DEPOSIT_AMOUNT_ETH
    );
    assert_eq!(
        token_eth.balance(&user),
        (INIT_USER_BALANCE - FIRST_DEPOSIT_AMOUNT_ETH - SECOND_DEPOSIT_AMOUNT_ETH) as i128
    );
    assert_eq!(
        token_eth.balance(&vault_contract_address),
        (CONTRACT_RESERVES_ETH + FIRST_DEPOSIT_AMOUNT_ETH + SECOND_DEPOSIT_AMOUNT_ETH) as i128
    );

    (
        margin_contract_client,
        token_eth,
        admin,
        user,
        vault_contract_address,
    )
}

#[test]
fn test_successful_deposits_of_one_token() {
    let (_margin_contract_client, _token_eth, _admin, _user, _vault_contract_address) =
        success_deposit_of_diff_token_with_prices();
}

#[test]
fn test_successful_redeem() {
    const TOKENS_DECIMALS: u32 = 18;

    const INIT_USER_BALANCE: u128 = 1000 * 10u128.pow(TOKENS_DECIMALS);
    const INIT_LIQUIDATOR_BALANCE_ETH: u128 = 1_000_000 * 10u128.pow(TOKENS_DECIMALS); // 1M ETH

    const CONTRACT_RESERVES_ETH: u128 = 1000 * 10u128.pow(TOKENS_DECIMALS);
    const CONTRACT_RESERVES_XLM: u128 = 1000 * 10u128.pow(TOKENS_DECIMALS);

    const FIRST_DEPOSIT_AMOUNT_ETH: u128 = 200 * 10u128.pow(TOKENS_DECIMALS);
    const SECOND_DEPOSIT_AMOUNT_ETH: u128 = 300 * 10u128.pow(TOKENS_DECIMALS);

    const PERCENT_DECIMALS: u32 = 5;

    const PRICE_DECIMALS: u32 = 8;
    const PRICE_ETH: u128 = 2000 * 10u128.pow(PRICE_DECIMALS);
    const PRICE_XLM: u128 = 10 * 10u128.pow(PRICE_DECIMALS);

    let (margin_contract_client, token_eth, admin, user, vault_contract_address) =
        success_deposit_of_diff_token_with_prices();

    let available_to_redeem: u128 =
        margin_contract_client.get_available_to_redeem(&user, &symbol_short!("eth"));

    assert_eq!(
        FIRST_DEPOSIT_AMOUNT_ETH + SECOND_DEPOSIT_AMOUNT_ETH,
        available_to_redeem
    );

    margin_contract_client.redeem(&user, &symbol_short!("eth"), &SECOND_DEPOSIT_AMOUNT_ETH);

    let user_deposited_balance = margin_contract_client.get_deposit(&user, &symbol_short!("eth"));

    assert_eq!(user_deposited_balance, FIRST_DEPOSIT_AMOUNT_ETH);
    assert_eq!(
        token_eth.balance(&user),
        (INIT_USER_BALANCE - FIRST_DEPOSIT_AMOUNT_ETH) as i128
    );
    assert_eq!(
        token_eth.balance(&vault_contract_address),
        (CONTRACT_RESERVES_ETH + FIRST_DEPOSIT_AMOUNT_ETH) as i128
    );

    let available_to_redeem: u128 =
        margin_contract_client.get_available_to_redeem(&user, &symbol_short!("eth"));

    assert_eq!(FIRST_DEPOSIT_AMOUNT_ETH, available_to_redeem);

    margin_contract_client.redeem(&user, &symbol_short!("eth"), &FIRST_DEPOSIT_AMOUNT_ETH);

    let user_deposited_balance = margin_contract_client.get_deposit(&user, &symbol_short!("eth"));

    assert_eq!(user_deposited_balance, 0);
    assert_eq!(token_eth.balance(&user), INIT_USER_BALANCE as i128);
    assert_eq!(
        token_eth.balance(&vault_contract_address),
        CONTRACT_RESERVES_ETH as i128
    );

    let available_to_redeem: u128 =
        margin_contract_client.get_available_to_redeem(&user, &symbol_short!("eth"));

    assert_eq!(0, available_to_redeem);
}
