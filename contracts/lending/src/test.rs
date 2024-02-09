#![cfg(test)]

extern crate std;

use crate::contract::{LendingContract, LendingContractClient};
use crate::storage::*;
use std::println;

use soroban_sdk::testutils::{Address as _, Events, Ledger, LedgerInfo};
use soroban_sdk::token::Client;
use soroban_sdk::{symbol_short, token, Address, Env, IntoVal, String, Symbol};

mod token_contract {
    soroban_sdk::contractimport!(file = "./token/soroban_token_contract.optimized.wasm");
}

fn create_custom_token<'a>(
    e: &Env,
    admin: &Address,
    name: &str,
    symbol: &str,
    decimals: &u32,
) -> token_contract::Client<'a> {
    let token_id = &e.register_contract_wasm(None, token_contract::WASM);
    let token = token_contract::Client::new(e, &token_id);
    token.initialize(
        admin,
        decimals,
        &String::from_slice(&e, name),
        &String::from_slice(&e, symbol),
    );
    token
}

pub fn success_deposit_of_one_token_setup() -> (LendingContractClient<'static>, Address, Address) {
    const TOKENS_DECIMALS: u32 = 18;

    const INIT_USER_BALANCE: u128 = 1000 * 10u128.pow(TOKENS_DECIMALS);
    const INIT_LIQUIDATOR_BALANCE_ETH: u128 = 1_000_000 * 10u128.pow(TOKENS_DECIMALS); // 1M ETH

    const CONTRACT_RESERVES: u128 = 1000000 * 10u128.pow(TOKENS_DECIMALS);
    const FIRST_DEPOSIT_AMOUNT_ETH: u128 = 200 * 10u128.pow(TOKENS_DECIMALS);
    const SECOND_DEPOSIT_AMOUNT_ETH: u128 = 300 * 10u128.pow(TOKENS_DECIMALS);

    const PERCENT_DECIMALS: u32 = 5;
    const LTV_ETH: u128 = 85 * 10u128.pow(PERCENT_DECIMALS); // 85%
    const LIQUIDATION_THRESHOLD_ETH: u128 = 90 * 10u128.pow(PERCENT_DECIMALS); // 90%
    const LTV_XLM: u128 = 75 * 10u128.pow(PERCENT_DECIMALS); // 75%
    const LIQUIDATION_THRESHOLD_XLM: u128 = 80 * 10u128.pow(PERCENT_DECIMALS); // 80%

    const INTEREST_RATE_DECIMALS: u32 = 18;
    const MIN_INTEREST_RATE: u128 = 5 * 10u128.pow(INTEREST_RATE_DECIMALS);
    const SAFE_BORROW_MAX_RATE: u128 = 30 * 10u128.pow(INTEREST_RATE_DECIMALS);
    const RATE_GROWTH_FACTOR: u128 = 70 * 10u128.pow(INTEREST_RATE_DECIMALS);

    const OPTIMAL_UTILIZATION_RATIO: u128 = 80 * 10u128.pow(PERCENT_DECIMALS);

    let env = Env::default();
    env.mock_all_auths();

    env.budget().reset_unlimited();
    let lending_contract_address = env.register_contract(None, LendingContract);
    let lending_contract_client = LendingContractClient::new(&env, &lending_contract_address);
    let admin = Address::generate(&env);
    let user1 = Address::generate(&env);
    let liquidator = Address::generate(&env);

    lending_contract_client.initialize(&admin, &liquidator);

    let token_xlm = create_custom_token(&env, &admin, "Xlm", "xlm", &TOKENS_DECIMALS);
    let token_eth = create_custom_token(&env, &admin, "Eth", "eth", &TOKENS_DECIMALS);

    // token_xlm.mint(&user1, &i128::try_from(INIT_USER_BALANCE).unwrap());
    token_eth.mint(&user1, &i128::try_from(INIT_USER_BALANCE).unwrap());

    // token_xlm.mint(&admin, &i128::try_from(CONTRACT_RESERVES * 100).unwrap());
    token_eth.mint(&admin, &i128::try_from(CONTRACT_RESERVES).unwrap());

    token_eth.mint(
        &liquidator,
        &i128::try_from(INIT_LIQUIDATOR_BALANCE_ETH).unwrap(),
    );

    lending_contract_client.add_markets(
        &symbol_short!("xlm"),
        &token_xlm.address,
        &symbol_short!("Xlm"),
        &TOKENS_DECIMALS,
        &LTV_XLM,
        &LIQUIDATION_THRESHOLD_XLM,
        &MIN_INTEREST_RATE,
        &SAFE_BORROW_MAX_RATE,
        &RATE_GROWTH_FACTOR,
        &OPTIMAL_UTILIZATION_RATIO,
    );

    lending_contract_client.add_markets(
        &symbol_short!("eth"),
        &token_eth.address,
        &symbol_short!("Eth"),
        &TOKENS_DECIMALS,
        &LTV_ETH,
        &LIQUIDATION_THRESHOLD_ETH,
        &MIN_INTEREST_RATE,
        &SAFE_BORROW_MAX_RATE,
        &RATE_GROWTH_FACTOR,
        &OPTIMAL_UTILIZATION_RATIO,
    );

    // Funding contract
    // token_xlm.transfer(&admin, &lending_contract_address, &i128::try_from(CONTRACT_RESERVES).unwrap());
    token_eth.transfer(
        &admin,
        &lending_contract_address,
        &i128::try_from(CONTRACT_RESERVES).unwrap(),
    );

    // lending_contract_client.update_price(&symbol_short!("xlm"), &PRICE_XLM);
    // lending_contract_client.update_price(&symbol_short!("eth"), &PRICE_ETH);

    lending_contract_client.deposit(&user1, &symbol_short!("eth"), &FIRST_DEPOSIT_AMOUNT_ETH);
    // lending_contract_client.deposit(&admin, &symbol_short!("eth"), &(FIRST_DEPOSIT_AMOUNT_ETH * 15 / 10));

    // lending_contract_client.toggle_collateral_setting(&user1, &symbol_short!("eth"));
    // lending_contract_client.toggle_collateral_setting(&admin, &symbol_short!("eth"));

    let mut user_deposited_balance: u128 =
        lending_contract_client.get_deposit(&user1, &symbol_short!("eth"));

    assert_eq!(user_deposited_balance, FIRST_DEPOSIT_AMOUNT_ETH);
    assert_eq!(
        token_eth.balance(&user1),
        (INIT_USER_BALANCE - FIRST_DEPOSIT_AMOUNT_ETH) as i128
    );
    assert_eq!(
        token_eth.balance(&lending_contract_address),
        (CONTRACT_RESERVES + FIRST_DEPOSIT_AMOUNT_ETH) as i128
    );

    lending_contract_client.deposit(&user1, &symbol_short!("eth"), &SECOND_DEPOSIT_AMOUNT_ETH);
    // lending_contract_client.borrow(&admin, &symbol_short!("eth"), &(SECOND_DEPOSIT_AMOUNT_ETH / 2));

    user_deposited_balance = lending_contract_client.get_deposit(&user1, &symbol_short!("eth"));

    assert_eq!(
        user_deposited_balance,
        FIRST_DEPOSIT_AMOUNT_ETH + SECOND_DEPOSIT_AMOUNT_ETH
    );
    assert_eq!(
        token_eth.balance(&user1),
        (INIT_USER_BALANCE - FIRST_DEPOSIT_AMOUNT_ETH - SECOND_DEPOSIT_AMOUNT_ETH) as i128
    );
    assert_eq!(
        token_eth.balance(&lending_contract_address),
        (CONTRACT_RESERVES + FIRST_DEPOSIT_AMOUNT_ETH + SECOND_DEPOSIT_AMOUNT_ETH) as i128
    );

    (lending_contract_client, admin, user1)
}

pub fn success_deposit_of_diff_token_with_prices(
) -> (Env, LendingContractClient<'static>, Address, Address) {
    const TOKENS_DECIMALS: u32 = 18;

    const INIT_BALANCE_ETH: u128 = 1000 * 10u128.pow(TOKENS_DECIMALS);
    const INIT_BALANCE_XLM: u128 = 1_000_000 * 10u128.pow(TOKENS_DECIMALS); // 1M XLM

    const INIT_LIQUIDATOR_BALANCE_ETH: u128 = 1_000_000 * 10u128.pow(TOKENS_DECIMALS); // 1M ETH
    const INIT_LIQUIDATOR_BALANCE_XLM: u128 = 1_000_000 * 10u128.pow(TOKENS_DECIMALS); // 1M XLM

    const DEPOSIT_AMOUNT_ETH: u128 = 200 * 10u128.pow(TOKENS_DECIMALS);
    const DEPOSIT_AMOUNT_XLM: u128 = 300 * 10u128.pow(TOKENS_DECIMALS);

    const CONTRACT_RESERVES_ETH: u128 = 1000 * 10u128.pow(TOKENS_DECIMALS);
    const CONTRACT_RESERVES_XLM: u128 = 1000 * 10u128.pow(TOKENS_DECIMALS);

    const PERCENT_DECIMALS: u32 = 5;
    const LTV_ETH: u128 = 85 * 10u128.pow(PERCENT_DECIMALS); // 85%
    const LIQUIDATION_THRESHOLD_ETH: u128 = 90 * 10u128.pow(PERCENT_DECIMALS); // 90%
    const LTV_XLM: u128 = 75 * 10u128.pow(PERCENT_DECIMALS); // 75%
    const LIQUIDATION_THRESHOLD_XLM: u128 = 80 * 10u128.pow(PERCENT_DECIMALS); // 80%

    const INTEREST_RATE_DECIMALS: u32 = 18;
    const MIN_INTEREST_RATE: u128 = 5 * 10u128.pow(INTEREST_RATE_DECIMALS);
    const SAFE_BORROW_MAX_RATE: u128 = 30 * 10u128.pow(INTEREST_RATE_DECIMALS);
    const RATE_GROWTH_FACTOR: u128 = 70 * 10u128.pow(INTEREST_RATE_DECIMALS);

    const OPTIMAL_UTILIZATION_RATIO: u128 = 80 * 10u128.pow(PERCENT_DECIMALS);

    const PRICE_DECIMALS: u32 = 8;
    const PRICE_ETH: u128 = 2000 * 10u128.pow(PRICE_DECIMALS);
    const PRICE_XLM: u128 = 10 * 10u128.pow(PRICE_DECIMALS);

    let env = Env::default();
    env.mock_all_auths();

    env.budget().reset_unlimited();
    let lending_contract_address = env.register_contract(None, LendingContract);
    let lending_contract_client = LendingContractClient::new(&env, &lending_contract_address);
    let admin = Address::generate(&env);
    let user1 = Address::generate(&env);
    let liquidator = Address::generate(&env);

    lending_contract_client.initialize(&admin, &liquidator);

    let token_xlm = create_custom_token(&env, &admin, "Xlm", "xlm", &TOKENS_DECIMALS);
    let token_eth = create_custom_token(&env, &admin, "Eth", "eth", &TOKENS_DECIMALS);

    token_xlm.mint(&user1, &i128::try_from(INIT_BALANCE_XLM).unwrap());
    token_eth.mint(&user1, &i128::try_from(INIT_BALANCE_ETH).unwrap());

    token_xlm.mint(&admin, &i128::try_from(INIT_BALANCE_XLM).unwrap());
    token_eth.mint(&admin, &i128::try_from(INIT_BALANCE_ETH).unwrap());

    token_xlm.mint(
        &liquidator,
        &i128::try_from(INIT_LIQUIDATOR_BALANCE_XLM).unwrap(),
    );
    token_eth.mint(
        &liquidator,
        &i128::try_from(INIT_LIQUIDATOR_BALANCE_ETH).unwrap(),
    );

    lending_contract_client.add_markets(
        &symbol_short!("xlm"),
        &token_xlm.address,
        &symbol_short!("Xlm"),
        &TOKENS_DECIMALS,
        &LTV_XLM,
        &LIQUIDATION_THRESHOLD_XLM,
        &MIN_INTEREST_RATE,
        &SAFE_BORROW_MAX_RATE,
        &RATE_GROWTH_FACTOR,
        &OPTIMAL_UTILIZATION_RATIO,
    );

    lending_contract_client.add_markets(
        &symbol_short!("eth"),
        &token_eth.address,
        &symbol_short!("Eth"),
        &TOKENS_DECIMALS,
        &LTV_ETH,
        &LIQUIDATION_THRESHOLD_ETH,
        &MIN_INTEREST_RATE,
        &SAFE_BORROW_MAX_RATE,
        &RATE_GROWTH_FACTOR,
        &OPTIMAL_UTILIZATION_RATIO,
    );

    // Funding contract
    token_xlm.transfer(
        &admin,
        &lending_contract_address,
        &i128::try_from(CONTRACT_RESERVES_XLM).unwrap(),
    );
    token_eth.transfer(
        &admin,
        &lending_contract_address,
        &i128::try_from(CONTRACT_RESERVES_ETH).unwrap(),
    );

    lending_contract_client.update_price(&symbol_short!("xlm"), &PRICE_XLM);
    lending_contract_client.update_price(&symbol_short!("eth"), &PRICE_ETH);

    let get_price_xlm: u128 = lending_contract_client.get_price(&symbol_short!("xlm"));
    let get_price_eth: u128 = lending_contract_client.get_price(&symbol_short!("eth"));

    assert_eq!(get_price_xlm, 1000000000); // 10$
    assert_eq!(get_price_eth, 200000000000); // 2000$

    println!("     ETH address: {:?}", token_eth.address);
    println!("    user address: {:?}", user1);
    println!("User ETH balance: {:?}", token_eth.balance(&user1));

    lending_contract_client.deposit(&user1, &symbol_short!("eth"), &DEPOSIT_AMOUNT_ETH);

    println!("User ETH balance: {:?}", token_eth.balance(&user1));

    let mut user_deposited_balance: u128 =
        lending_contract_client.get_deposit(&user1, &symbol_short!("eth"));

    assert_eq!(user_deposited_balance, DEPOSIT_AMOUNT_ETH);
    assert_eq!(
        token_eth.balance(&user1),
        (INIT_BALANCE_ETH - DEPOSIT_AMOUNT_ETH) as i128
    );
    assert_eq!(
        token_eth.balance(&lending_contract_address),
        (CONTRACT_RESERVES_ETH + DEPOSIT_AMOUNT_ETH) as i128
    );

    lending_contract_client.deposit(&user1, &symbol_short!("xlm"), &DEPOSIT_AMOUNT_XLM);

    user_deposited_balance = lending_contract_client.get_deposit(&user1, &symbol_short!("xlm"));

    assert_eq!(user_deposited_balance, DEPOSIT_AMOUNT_XLM);
    assert_eq!(
        token_xlm.balance(&user1),
        (INIT_BALANCE_XLM - DEPOSIT_AMOUNT_XLM) as i128
    );
    assert_eq!(
        token_xlm.balance(&lending_contract_address),
        (CONTRACT_RESERVES_XLM + DEPOSIT_AMOUNT_XLM) as i128
    );

    (env, lending_contract_client, admin, user1)
}

pub fn success_deposit_as_collateral_of_diff_token_with_prices(
) -> (Env, LendingContractClient<'static>, Address, Address) {
    let (env, lending_contract_client, admin, user) = success_deposit_of_diff_token_with_prices();

    lending_contract_client.toggle_collateral_setting(&user, &symbol_short!("eth"));
    lending_contract_client.toggle_collateral_setting(&user, &symbol_short!("xlm"));

    lending_contract_client.toggle_collateral_setting(&admin, &symbol_short!("eth"));
    lending_contract_client.toggle_collateral_setting(&admin, &symbol_short!("xlm"));

    (env, lending_contract_client, admin, user)
}

pub fn success_borrow_setup() -> (
    Env,
    LendingContractClient<'static>,
    Address,
    Address,
    Address,
    token_contract::Client<'static>,
    token_contract::Client<'static>,
) {
    const TOKENS_DECIMALS: u32 = 18;

    const INIT_BALANCE_ETH: u128 = 10_000 * 10u128.pow(TOKENS_DECIMALS); // 10_000 ETH
    const INIT_BALANCE_XLM: u128 = 10_000 * 10u128.pow(TOKENS_DECIMALS); // 10_000 XLM
    const INIT_BALANCE_USDT: u128 = 10_000 * 10u128.pow(TOKENS_DECIMALS); // 10_000 USDT

    const INIT_LIQUIDATOR_BALANCE_ETH: u128 = 1_000_000 * 10u128.pow(TOKENS_DECIMALS); // 1M ETH
    const INIT_LIQUIDATOR_BALANCE_XLM: u128 = 1_000_000 * 10u128.pow(TOKENS_DECIMALS); // 1M XLM

    const DEPOSIT_AMOUNT_ETH: u128 = 200 * 10u128.pow(TOKENS_DECIMALS);
    const DEPOSIT_AMOUNT_XLM: u128 = 300 * 10u128.pow(TOKENS_DECIMALS);

    const CONTRACT_RESERVES_ETH: u128 = 1000 * 10u128.pow(TOKENS_DECIMALS);
    const CONTRACT_RESERVES_XLM: u128 = 1000 * 10u128.pow(TOKENS_DECIMALS);

    const BORROW_AMOUNT_ETH: u128 = 50 * 10u128.pow(TOKENS_DECIMALS);

    const PERCENT_DECIMALS: u32 = 5;
    const LTV_ETH: u128 = 85 * 10u128.pow(PERCENT_DECIMALS); // 85%
    const LIQUIDATION_THRESHOLD_ETH: u128 = 90 * 10u128.pow(PERCENT_DECIMALS); // 90%
    const LTV_XLM: u128 = 75 * 10u128.pow(PERCENT_DECIMALS); // 75%
    const LIQUIDATION_THRESHOLD_XLM: u128 = 80 * 10u128.pow(PERCENT_DECIMALS); // 80%

    const INTEREST_RATE_DECIMALS: u32 = 18;
    const MIN_INTEREST_RATE: u128 = 5 * 10u128.pow(INTEREST_RATE_DECIMALS);
    const SAFE_BORROW_MAX_RATE: u128 = 30 * 10u128.pow(INTEREST_RATE_DECIMALS);
    const RATE_GROWTH_FACTOR: u128 = 70 * 10u128.pow(INTEREST_RATE_DECIMALS);

    const OPTIMAL_UTILIZATION_RATIO: u128 = 80 * 10u128.pow(PERCENT_DECIMALS);

    const PRICE_DECIMALS: u32 = 8;
    const PRICE_ETH: u128 = 2000 * 10u128.pow(PRICE_DECIMALS);
    const PRICE_XLM: u128 = 10 * 10u128.pow(PRICE_DECIMALS);

    let env = Env::default();
    env.mock_all_auths();

    env.budget().reset_unlimited();
    let lending_contract_address = env.register_contract(None, LendingContract);
    let lending_contract_client = LendingContractClient::new(&env, &lending_contract_address);
    let admin = Address::generate(&env);
    let user1 = Address::generate(&env);
    let liquidator = Address::generate(&env);

    lending_contract_client.initialize(&admin, &liquidator);

    let token_xlm = create_custom_token(&env, &admin, "Xlm", "xlm", &TOKENS_DECIMALS);
    let token_eth = create_custom_token(&env, &admin, "Eth", "eth", &TOKENS_DECIMALS);
    let token_usdt = create_custom_token(&env, &admin, "USDT", "usdt", &TOKENS_DECIMALS);

    token_xlm.mint(&user1, &i128::try_from(INIT_BALANCE_XLM).unwrap());
    token_eth.mint(&user1, &i128::try_from(INIT_BALANCE_ETH).unwrap());
    token_usdt.mint(&user1, &i128::try_from(INIT_BALANCE_ETH).unwrap());

    token_xlm.mint(&admin, &i128::try_from(CONTRACT_RESERVES_ETH).unwrap());
    token_eth.mint(&admin, &i128::try_from(CONTRACT_RESERVES_ETH).unwrap());

    token_xlm.mint(
        &liquidator,
        &i128::try_from(INIT_LIQUIDATOR_BALANCE_XLM).unwrap(),
    );
    token_eth.mint(
        &liquidator,
        &i128::try_from(INIT_LIQUIDATOR_BALANCE_ETH).unwrap(),
    );

    lending_contract_client.add_markets(
        &symbol_short!("xlm"),
        &token_xlm.address,
        &symbol_short!("Xlm"),
        &TOKENS_DECIMALS,
        &LTV_XLM,
        &LIQUIDATION_THRESHOLD_XLM,
        &MIN_INTEREST_RATE,
        &SAFE_BORROW_MAX_RATE,
        &RATE_GROWTH_FACTOR,
        &OPTIMAL_UTILIZATION_RATIO,
    );

    lending_contract_client.add_markets(
        &symbol_short!("eth"),
        &token_eth.address,
        &symbol_short!("Eth"),
        &TOKENS_DECIMALS,
        &LTV_ETH,
        &LIQUIDATION_THRESHOLD_ETH,
        &MIN_INTEREST_RATE,
        &SAFE_BORROW_MAX_RATE,
        &RATE_GROWTH_FACTOR,
        &OPTIMAL_UTILIZATION_RATIO,
    );

    // Funding contract
    token_xlm.transfer(
        &admin,
        &lending_contract_address,
        &i128::try_from(CONTRACT_RESERVES_XLM).unwrap(),
    );
    token_eth.transfer(
        &admin,
        &lending_contract_address,
        &i128::try_from(CONTRACT_RESERVES_ETH).unwrap(),
    );

    lending_contract_client.update_price(&symbol_short!("xlm"), &PRICE_XLM);
    lending_contract_client.update_price(&symbol_short!("eth"), &PRICE_ETH);

    let get_price_xlm: u128 = lending_contract_client.get_price(&symbol_short!("xlm"));
    let get_price_eth: u128 = lending_contract_client.get_price(&symbol_short!("eth"));

    assert_eq!(get_price_xlm, 1000000000); // 10$
    assert_eq!(get_price_eth, 200000000000); // 2000$

    lending_contract_client.toggle_collateral_setting(&user1, &symbol_short!("eth"));
    lending_contract_client.toggle_collateral_setting(&user1, &symbol_short!("xlm"));

    lending_contract_client.toggle_collateral_setting(&admin, &symbol_short!("eth"));
    lending_contract_client.toggle_collateral_setting(&admin, &symbol_short!("xlm"));

    lending_contract_client.deposit(&user1, &symbol_short!("eth"), &DEPOSIT_AMOUNT_ETH);

    let mut current_info: LedgerInfo = env.ledger().get();
    current_info.timestamp = 1000;
    env.ledger().set(current_info);

    let _available_to_redeem: u128 =
        lending_contract_client.get_available_to_redeem(&user1, &symbol_short!("eth"));

    let user_deposited_balance: u128 =
        lending_contract_client.get_deposit(&user1, &symbol_short!("eth"));

    assert_eq!(user_deposited_balance, DEPOSIT_AMOUNT_ETH);

    assert_eq!(
        token_eth.balance(&user1) as u128,
        INIT_BALANCE_ETH - DEPOSIT_AMOUNT_ETH
    );

    assert_eq!(
        token_eth.balance(&lending_contract_address) as u128,
        CONTRACT_RESERVES_ETH + DEPOSIT_AMOUNT_ETH
    );

    lending_contract_client.deposit(&user1, &symbol_short!("xlm"), &DEPOSIT_AMOUNT_XLM);

    let mut current_info = env.ledger().get();
    current_info.timestamp = 2000;
    env.ledger().set(current_info);

    let user_deposited_balance: u128 =
        lending_contract_client.get_deposit(&user1, &symbol_short!("xlm"));

    assert_eq!(user_deposited_balance, DEPOSIT_AMOUNT_XLM);

    assert_eq!(
        token_xlm.balance(&user1) as u128,
        INIT_BALANCE_XLM - DEPOSIT_AMOUNT_XLM
    );

    assert_eq!(
        token_xlm.balance(&lending_contract_address) as u128,
        CONTRACT_RESERVES_XLM + DEPOSIT_AMOUNT_XLM
    );
    let mut current_info = env.ledger().get();
    current_info.timestamp = 10000;
    env.ledger().set(current_info);

    lending_contract_client.borrow(&user1, &symbol_short!("eth"), &BORROW_AMOUNT_ETH);

    (
        env,
        lending_contract_client,
        admin,
        user1,
        liquidator,
        token_xlm,
        token_eth,
    )
}

#[test]
fn test_successful_deposits_of_one_token() {
    const TOKENS_DECIMALS: u32 = 18;

    const INIT_USER_BALANCE: u128 = 1000 * 10u128.pow(TOKENS_DECIMALS);
    const INIT_LIQUIDATOR_BALANCE_ETH: u128 = 1_000_000 * 10u128.pow(TOKENS_DECIMALS); // 1M ETH
    const INIT_LIQUIDATOR_BALANCE_XLM: u128 = 1_000_000 * 10u128.pow(TOKENS_DECIMALS); // 1M XLM

    const CONTRACT_RESERVES: u128 = 1_000_000 * 10u128.pow(TOKENS_DECIMALS);
    const FIRST_DEPOSIT_AMOUNT: u128 = 200 * 10u128.pow(TOKENS_DECIMALS);
    const SECOND_DEPOSIT_AMOUNT: u128 = 300 * 10u128.pow(TOKENS_DECIMALS);

    const PERCENT_DECIMALS: u32 = 5;
    const LTV_ETH: u128 = 85 * 10u128.pow(PERCENT_DECIMALS); // 85%
    const LIQUIDATION_THRESHOLD_ETH: u128 = 90 * 10u128.pow(PERCENT_DECIMALS); // 90%
    const LTV_XLM: u128 = 75 * 10u128.pow(PERCENT_DECIMALS); // 75%
    const LIQUIDATION_THRESHOLD_XLM: u128 = 80 * 10u128.pow(PERCENT_DECIMALS); // 80%

    const INTEREST_RATE_DECIMALS: u32 = 18;
    const MIN_INTEREST_RATE: u128 = 5 * 10u128.pow(INTEREST_RATE_DECIMALS);
    const SAFE_BORROW_MAX_RATE: u128 = 30 * 10u128.pow(INTEREST_RATE_DECIMALS);
    const RATE_GROWTH_FACTOR: u128 = 70 * 10u128.pow(INTEREST_RATE_DECIMALS);

    const OPTIMAL_UTILIZATION_RATIO: u128 = 80 * 10u128.pow(PERCENT_DECIMALS);

    const PRICE_DECIMALS: u32 = 8;
    const PRICE_ETH: u128 = 2000 * 10u128.pow(PRICE_DECIMALS);
    const PRICE_XLM: u128 = 10 * 10u128.pow(PRICE_DECIMALS);

    let env = Env::default();
    env.mock_all_auths();

    env.budget().reset_unlimited();
    let lending_contract_address = env.register_contract(None, LendingContract);
    let lending_contract_client = LendingContractClient::new(&env, &lending_contract_address);
    let admin = Address::generate(&env);
    let user1 = Address::generate(&env);
    let liquidator = Address::generate(&env);

    lending_contract_client.initialize(&admin, &liquidator);

    let token_xlm = create_custom_token(&env, &admin, "Xlm", "xlm", &TOKENS_DECIMALS);
    let token_eth = create_custom_token(&env, &admin, "Eth", "eth", &TOKENS_DECIMALS);

    token_xlm.mint(&user1, &i128::try_from(INIT_USER_BALANCE).unwrap());
    token_eth.mint(&user1, &i128::try_from(INIT_USER_BALANCE).unwrap());

    token_xlm.mint(&admin, &i128::try_from(CONTRACT_RESERVES * 100).unwrap());
    token_eth.mint(&admin, &i128::try_from(CONTRACT_RESERVES * 100).unwrap());

    lending_contract_client.add_markets(
        &symbol_short!("xlm"),
        &token_xlm.address,
        &symbol_short!("Xlm"),
        &TOKENS_DECIMALS,
        &LTV_XLM,
        &LIQUIDATION_THRESHOLD_XLM,
        &5000000000000000000,
        &20000000000000000000,
        &100000000000000000000,
        &OPTIMAL_UTILIZATION_RATIO,
    );

    lending_contract_client.add_markets(
        &symbol_short!("eth"),
        &token_eth.address,
        &symbol_short!("Eth"),
        &TOKENS_DECIMALS,
        &LTV_ETH,
        &LIQUIDATION_THRESHOLD_ETH,
        &MIN_INTEREST_RATE,
        &SAFE_BORROW_MAX_RATE,
        &RATE_GROWTH_FACTOR,
        &OPTIMAL_UTILIZATION_RATIO,
    );

    // Funding contract
    token_xlm.transfer(
        &admin,
        &lending_contract_address,
        &i128::try_from(CONTRACT_RESERVES / 100).unwrap(),
    );
    token_eth.transfer(
        &admin,
        &lending_contract_address,
        &i128::try_from(CONTRACT_RESERVES / 100).unwrap(),
    );

    lending_contract_client.update_price(&symbol_short!("xlm"), &PRICE_XLM);
    lending_contract_client.update_price(&symbol_short!("eth"), &PRICE_ETH);

    lending_contract_client.deposit(&user1, &symbol_short!("eth"), &FIRST_DEPOSIT_AMOUNT);
    lending_contract_client.deposit(
        &admin,
        &symbol_short!("eth"),
        &(FIRST_DEPOSIT_AMOUNT * 15 / 10),
    );

    lending_contract_client.toggle_collateral_setting(&user1, &symbol_short!("eth"));
    lending_contract_client.toggle_collateral_setting(&admin, &symbol_short!("eth"));

    let mut user_deposited_balance: u128 =
        lending_contract_client.get_deposit(&user1, &symbol_short!("eth"));

    assert_eq!(user_deposited_balance, FIRST_DEPOSIT_AMOUNT);
    assert_eq!(
        token_eth.balance(&user1),
        (INIT_USER_BALANCE - FIRST_DEPOSIT_AMOUNT) as i128
    );

    lending_contract_client.deposit(&user1, &symbol_short!("eth"), &SECOND_DEPOSIT_AMOUNT);
    lending_contract_client.borrow(&admin, &symbol_short!("eth"), &(SECOND_DEPOSIT_AMOUNT / 2));

    let mut current_info: LedgerInfo = env.ledger().get();

    println!("Current ledger info: {:?}", current_info);
    current_info.timestamp = 31536000;
    env.ledger().set(current_info);

    println!("New timestamp: {:?}", env.ledger().timestamp());

    let total_borrow_data: TotalBorrowData =
        lending_contract_client.get_total_borrow_data(&symbol_short!("eth"));
    println!("Total borrow data: {:?}", total_borrow_data);

    let reserves_by_token: u128 =
        lending_contract_client.get_total_reserves_by_token(&symbol_short!("eth"));
    println!("Total Reserves for Eth : {:?}", reserves_by_token);

    user_deposited_balance = lending_contract_client.get_deposit(&user1, &symbol_short!("eth"));
    println!(
        "User initial deposit       : {:?}",
        FIRST_DEPOSIT_AMOUNT + SECOND_DEPOSIT_AMOUNT
    );
    println!("User deposit after set time: {:?}", user_deposited_balance);
    assert!(user_deposited_balance > FIRST_DEPOSIT_AMOUNT + SECOND_DEPOSIT_AMOUNT);
    assert_eq!(
        token_eth.balance(&user1),
        (INIT_USER_BALANCE - FIRST_DEPOSIT_AMOUNT - SECOND_DEPOSIT_AMOUNT) as i128
    );
}

#[test]
fn test_get_deposit() {
    let (lending_contract_client, admin, user) = success_deposit_of_one_token_setup();

    let user_deposit_amount_eth: u128 =
        lending_contract_client.get_deposit(&user, &symbol_short!("eth"));
    let user_deposit_amount_xlm: u128 =
        lending_contract_client.get_deposit(&user, &symbol_short!("xlm"));

    assert_eq!(user_deposit_amount_xlm, 0); // 0
    assert_eq!(user_deposit_amount_eth, 500000000000000000000); // 500
}

#[test]
fn test_get_mm_token_price() {
    let (lending_contract_client, admin, user) = success_deposit_of_one_token_setup();

    let get_mm_token_price_eth: u128 =
        lending_contract_client.get_mm_token_price(&symbol_short!("eth"));
    let get_mm_token_price_xlm: u128 =
        lending_contract_client.get_mm_token_price(&symbol_short!("xlm"));

    assert_eq!(get_mm_token_price_xlm, 1000000000000000000); // 1:1
    assert_eq!(get_mm_token_price_eth, 1000000000000000000); // 1:1
}

#[test]
fn test_get_liquidity_rate() {
    // contract reserves: 1000 ETH and 1000 XLM
    // user deposited 200 ETH and 300 XLM
    let (env, lending_contract_client, admin, user) =
        success_deposit_as_collateral_of_diff_token_with_prices();

    const DECIMAL_FRACTIONAL: u128 = 1_000_000_000_000_000_000_u128; // 1*10**18
    const BORROW_SECOND_TOKEN_FIRST_PART: u128 = 300 * DECIMAL_FRACTIONAL;

    lending_contract_client.borrow(
        &user,
        &symbol_short!("xlm"),
        &BORROW_SECOND_TOKEN_FIRST_PART,
    );

    let get_liquidity_rate_eth: u128 =
        lending_contract_client.get_liquidity_rate(&symbol_short!("eth"));
    let get_liquidity_rate_xlm: u128 =
        lending_contract_client.get_liquidity_rate(&symbol_short!("xlm"));

    assert_eq!(get_liquidity_rate_xlm, 1153846153846153846); // ~1.154%
    assert_eq!(get_liquidity_rate_eth, 0);
}

#[test]
fn test_get_user_borrow_with_interest() {
    const TOKENS_DECIMALS: u32 = 18;
    const BORROW_AMOUNT_ETH: u128 = 50 * 10u128.pow(TOKENS_DECIMALS); // 50 ETH
    const BORROW_AMOUNT_XLM: u128 = 200 * 10u128.pow(TOKENS_DECIMALS); // 200 XLM

    const YEAR_IN_SECONDS: u64 = 31536000;

    // contract reserves: 1000 ETH and 1000 XLM
    // user deposited 200 ETH and 300 XLM
    let (env, lending_contract_client, admin, user) =
        success_deposit_as_collateral_of_diff_token_with_prices();

    let mut user_borrow_amount_with_interest_eth: u128 =
        lending_contract_client.get_user_borrow_with_interest(&user, &symbol_short!("eth"));

    let mut user_borrow_amount_with_interest_xlm: u128 =
        lending_contract_client.get_user_borrow_with_interest(&user, &symbol_short!("xlm"));

    // user hasn't borrowed anything yet
    assert_eq!(user_borrow_amount_with_interest_eth, 0);
    assert_eq!(user_borrow_amount_with_interest_xlm, 0);

    lending_contract_client.borrow(&user, &symbol_short!("eth"), &BORROW_AMOUNT_ETH);
    lending_contract_client.borrow(&user, &symbol_short!("xlm"), &BORROW_AMOUNT_XLM);

    user_borrow_amount_with_interest_eth =
        lending_contract_client.get_user_borrow_with_interest(&user, &symbol_short!("eth"));

    user_borrow_amount_with_interest_xlm =
        lending_contract_client.get_user_borrow_with_interest(&user, &symbol_short!("xlm"));

    assert_eq!(user_borrow_amount_with_interest_eth, 50000000000000000000); // 50 ETH
    assert_eq!(user_borrow_amount_with_interest_xlm, 200000000000000000000); // 200 XLM

    let mut current_info: LedgerInfo = env.ledger().get();
    current_info.timestamp = YEAR_IN_SECONDS;
    env.ledger().set(current_info);

    user_borrow_amount_with_interest_eth =
        lending_contract_client.get_user_borrow_with_interest(&user, &symbol_short!("eth"));

    user_borrow_amount_with_interest_xlm =
        lending_contract_client.get_user_borrow_with_interest(&user, &symbol_short!("xlm"));

    // 50 ETH + 5% borrow APY = 50 ETH + 2.5 ETH = 52.5 ETH
    assert_eq!(user_borrow_amount_with_interest_eth, 52500000000000000000);
    // 200 XLM + 5% borrow APY = 200 XLM + 10 XLM = 210 XLM
    assert_eq!(user_borrow_amount_with_interest_xlm, 210000000000000000000);

    // let users_with_borrow = lending_contract_client.GetAllUsersWithBorrows();

    // assert!(!users_with_borrow.is_empty());
}

#[test]
fn test_success_borrow_one_token() {
    const DECIMAL_FRACTIONAL: u128 = 1_000000_000000_000000_u128; // 1*10**18

    const INIT_BALANCE_SECOND_TOKEN: u128 = 1_000_000 * DECIMAL_FRACTIONAL; // 1M XLM

    const DEPOSIT_OF_SECOND_TOKEN: u128 = 300 * DECIMAL_FRACTIONAL;

    const BORROW_SECOND_TOKEN: u128 = 300 * DECIMAL_FRACTIONAL;

    /*
    price eth 1500
    price xlm 10

    deposited eth 200 * 1500 = 300_000 $

    borrowed xlm 300 * 10 = 3_000 $
    */

    // contract reserves: 1000 ETH and 1000 XLM
    // user deposited 200 ETH and 300 XLM
    let (env, lending_contract_client, admin, user) =
        success_deposit_as_collateral_of_diff_token_with_prices();

    lending_contract_client.redeem(&user, &symbol_short!("xlm"), &DEPOSIT_OF_SECOND_TOKEN);

    let user_deposited_balance_after_redeeming: u128 =
        lending_contract_client.get_deposit(&user, &symbol_short!("xlm"));

    assert_eq!(user_deposited_balance_after_redeeming, 0);

    // assert_eq!(
    //     app.wrap()
    //         .query_balance("user", "xlm")
    //         .unwrap()
    //         .amount
    //         ,
    //     INIT_BALANCE_SECOND_TOKEN
    // );

    lending_contract_client.borrow(&user, &symbol_short!("xlm"), &BORROW_SECOND_TOKEN);

    let mut current_info: LedgerInfo = env.ledger().get();
    current_info.timestamp = 31536000;
    env.ledger().set(current_info);

    let user_borrowed_balance: u128 =
        lending_contract_client.get_user_borrow_with_interest(&user, &symbol_short!("xlm"));

    assert_ne!(user_borrowed_balance, BORROW_SECOND_TOKEN);
    assert_eq!(user_borrowed_balance, BORROW_SECOND_TOKEN * 105 / 100);
}

#[test]
fn test_success_repay_whole_amount() {
    // user borrowed 50 ETH
    let (env, lending_contract_client, admin, user, liquidator, token_xlm, token_eth) =
        success_borrow_setup();

    let mut current_info: LedgerInfo = env.ledger().get();
    current_info.timestamp = 3153600 + 10000;
    env.ledger().set(current_info);

    let user_borrow_amount_with_interest: u128 =
        lending_contract_client.get_user_borrow_with_interest(&user, &symbol_short!("eth"));

    let amount_to_repay_with_interest = user_borrow_amount_with_interest;

    lending_contract_client.repay(&user, &symbol_short!("eth"), &amount_to_repay_with_interest);

    let user_borrow_amount_with_interest: u128 =
        lending_contract_client.get_user_borrow_with_interest(&user, &symbol_short!("eth"));

    assert_eq!(user_borrow_amount_with_interest, 0);
}

#[test]
fn test_success_repay_more_than_needed() {
    // user borrowed 50 ETH
    let (env, lending_contract_client, admin, user, liquidator, token_xlm, token_eth) =
        success_borrow_setup();

    let mut ledger_info: LedgerInfo = env.ledger().get();
    ledger_info.timestamp = 3153600 + 10000;
    env.ledger().set(ledger_info);

    let user_borrow_amount_with_interest: u128 =
        lending_contract_client.get_user_borrow_with_interest(&user, &symbol_short!("eth"));

    let amount_to_repay_with_interest: u128 = user_borrow_amount_with_interest;

    let underlying_balance_before_repay: i128 = token_eth.balance(&lending_contract_client.address);

    lending_contract_client.repay(
        &user,
        &symbol_short!("eth"),
        &(amount_to_repay_with_interest * 2),
    );

    let underlying_balance_after_repay: i128 = token_eth.balance(&lending_contract_client.address);

    // paying only what we supposed to, not twice as much
    assert_eq!(
        underlying_balance_after_repay - amount_to_repay_with_interest as i128,
        underlying_balance_before_repay
    );

    let user_borrow_amount_with_interest: u128 =
        lending_contract_client.get_user_borrow_with_interest(&user, &symbol_short!("eth"));

    assert_eq!(user_borrow_amount_with_interest, 0);
}

#[test]
fn test_success_repay_by_parts() {
    const TOKENS_DECIMALS: u32 = 18;
    const BORROW_AMOUNT_ETH: u128 = 50 * 10u128.pow(TOKENS_DECIMALS); // 50 ETH

    // user borrowed 50 ETH
    let (env, lending_contract_client, admin, user, liquidator, token_xlm, token_eth) =
        success_borrow_setup();

    let mut ledger_info: LedgerInfo = env.ledger().get();
    ledger_info.timestamp = 31536000 + 10000;
    env.ledger().set(ledger_info);

    let borrow_info_before_first_repay: u128 =
        lending_contract_client.get_user_borrow_with_interest(&user, &symbol_short!("eth"));

    assert_eq!(
        borrow_info_before_first_repay,
        BORROW_AMOUNT_ETH * 105 / 100
    );

    lending_contract_client.repay(
        &user,
        &symbol_short!("eth"),
        &(borrow_info_before_first_repay / 2),
    );

    let borrow_info_after_first_repay: u128 =
        lending_contract_client.get_user_borrow_with_interest(&user, &symbol_short!("eth"));

    lending_contract_client.repay(
        &user,
        &symbol_short!("eth"),
        &(borrow_info_after_first_repay),
    );

    let user_borrowed_balance: u128 =
        lending_contract_client.get_user_borrow_with_interest(&user, &symbol_short!("eth"));

    assert_eq!(user_borrowed_balance, 0);
}

// #[test]
// fn test_success_liquidation() {
//     const TOKENS_DECIMALS: u32 = 18;
//     const BORROW_AMOUNT_ETH: u128 = 121 * 10u128.pow(TOKENS_DECIMALS); // 121 ETH
//     const LIQUIDATOR_DEPOSIT_AMOUNT_ETH: u128 = 10_000 * 10u128.pow(TOKENS_DECIMALS); // 10_000 ETH
//     const YEAR_IN_SECONDS: u64 = 31536000;

//     // contract reserves: 1000 ETH
//     // user deposited 200 ETH and 300 XLM
//     // user borrowed 50 ETH
//     let (env, lending_contract_client, admin, user, liquidator, token_xlm, token_eth) =
//         success_borrow_setup();

//     let mut ledger_info: LedgerInfo = env.ledger().get();
//     ledger_info.timestamp = 10000;
//     env.ledger().set(ledger_info.clone());

//     let user_deposited_balance_eth: u128 = lending_contract_client.get_deposit(&user, &symbol_short!("eth"));

//     assert_eq!(user_deposited_balance_eth, 200_000000000000000000); // 200 ETH

//     let user_deposited_balance_xlm: u128 =
//         lending_contract_client.get_deposit(&user, &symbol_short!("xlm"));

//     assert_eq!(user_deposited_balance_xlm, 300_000000000000000000); // 300 XLM

//     let user_collateral_usd: u128 = lending_contract_client.get_user_collateral_usd(&user);

//     // 200 ETH * 2000 + 300 XLM * 10 == 403_000$
//     assert_eq!(user_collateral_usd, 403_00000000000);

//     let reserve_configuration_xlm: ReserveConfiguration =
//         lending_contract_client.get_reserve_configuration(&symbol_short!("xlm"));

//     assert_eq!(reserve_configuration_xlm.loan_to_value_ratio, 7500000); // ltv_xlm = 75%

//     let reserve_configuration_eth: ReserveConfiguration =
//         lending_contract_client.get_reserve_configuration(&symbol_short!("eth"));

//     assert_eq!(reserve_configuration_eth.loan_to_value_ratio, 8500000); // ltv_eth = 85%

//     let user_max_allowed_borrow_amount_usd: u128 =
//         lending_contract_client.get_user_max_allowed_borrow_usd(&user);

//     // 200 ETH * 0.85 * 2000 + 300 XLM * 0.75 * 10 == 340_000 + 2_250 = 342_250$
//     assert_eq!(user_max_allowed_borrow_amount_usd, 342_250_00000000);

//     let user_borrowed_usd: u128 = lending_contract_client.get_user_borrowed_usd(&user);

//     assert_eq!(user_borrowed_usd, 100_000_00000000); // 50 ETH * 2000 = 100_000$

//     let available_to_borrow_eth: u128 =
//         lending_contract_client.get_available_to_borrow(&user, &symbol_short!("eth"));

//     // (user_max_allowed_borrow_amount_usd - user_borrowed_usd) / price =
//     // (342_250$ - 100_000$) / price = 242_250$ / price
//     assert_eq!(available_to_borrow_eth, 121125000000000000000); // 242_250$ / 2000 == 121.125 ETH

//     lending_contract_client.borrow(&user, &symbol_short!("eth"), &BORROW_AMOUNT_ETH);

//     let available_to_borrow_eth: u128 =
//         lending_contract_client.get_available_to_borrow(&user, &symbol_short!("eth"));

//     assert_eq!(available_to_borrow_eth, 125000000000000000); // 0.125 ETH

//     let user_liquidation_threshold: u128 = lending_contract_client.get_user_liquidation_threshold(&user);
//     assert_eq!(user_liquidation_threshold, 8992555); // 89.92555%

//     let user_utilization_rate: u128 = lending_contract_client.GetUserUtilizationRate(&user);
//     assert_eq!(user_utilization_rate, 8486352); // 84.86352% < 89.92555%

//     ledger_info.timestamp = 2 * YEAR_IN_SECONDS + 10000; // after 2 years
//     env.ledger().set(ledger_info.clone());

//     let available_to_borrow_eth: u128 =
//         lending_contract_client.get_available_to_borrow(&user, &symbol_short!("eth"));
//     assert_eq!(available_to_borrow_eth, 0);

//     let user_liquidation_threshold: u128 = lending_contract_client.get_user_liquidation_threshold(&user);
//     assert_eq!(user_liquidation_threshold, 8992676); // 89.92676%

//     let user_utilization_rate: u128 = lending_contract_client.GetUserUtilizationRate(&user);
//     assert_eq!(user_utilization_rate, 9366274); // 93.66274% > 89.92676%

//     let user_deposit_amount_eth = lending_contract_client.get_deposit(&user, &symbol_short!("eth"));
//     let user_deposit_amount_xlm = lending_contract_client.get_deposit(&user, &symbol_short!("xlm"));

//     assert_eq!(user_deposit_amount_eth, 203_331286529000814400); // 203.331286529000814400 ETH
//     assert_eq!(user_deposit_amount_xlm, 300_000000000000000000); // 300 XLM

//     let user_borrow_amount_eth: u128 =
//         lending_contract_client.get_user_borrow_with_interest(&user, &symbol_short!("eth"));

//     assert_eq!(user_borrow_amount_eth, 191850604584630250327); // 191.850604584630250327 ETH

//     lending_contract_client.deposit(
//         &liquidator,
//         &symbol_short!("eth"),
//         &LIQUIDATOR_DEPOSIT_AMOUNT_ETH,
//     );

//     let liquidator_deposit_amount_eth =
//         lending_contract_client.get_deposit(&liquidator, &symbol_short!("eth"));

//     let liquidator_deposit_amount_xlm =
//         lending_contract_client.get_deposit(&liquidator, &symbol_short!("xlm"));

//     // TODO: need to correct the calculation inaccuracy
//     assert_eq!(liquidator_deposit_amount_eth, 9999999999999999999999); // 9999.999999999999999999 ETH
//     assert_eq!(liquidator_deposit_amount_xlm, 0); // 0

//     lending_contract_client.Liquidation(&user);

//     let user_collateral_usd: u128 = lending_contract_client.get_user_collateral_usd(&user);

//     // after liquidation, all collateral is transferred to the liquidator
//     assert_eq!(user_collateral_usd, 0);

//     let user_borrowed_usd: u128 = lending_contract_client.get_user_borrowed_usd(&user);

//     // after liquidation, all borrowings are repaid by the liquidator
//     assert_eq!(user_borrowed_usd, 0);

//     let liquidator_deposit_amount_eth =
//         lending_contract_client.get_deposit(&liquidator, &symbol_short!("eth"));
//     let liquidator_deposit_amount_xlm =
//         lending_contract_client.get_deposit(&liquidator, &symbol_short!("xlm"));

//     // 9999.999999999999999999 ETH - 191.850604584630250327 ETH + 203.331286529000814400 ETH ~= 10011,480681944 ETH
//     // TODO: need to correct the calculation inaccuracy
//     assert_eq!(liquidator_deposit_amount_eth, 10008510511955314159271); // 10008.510511955314159271 ETH
//     assert_eq!(liquidator_deposit_amount_xlm, 300000000000000000000); // 300 XLM
// }

#[test]
fn test_full_borrow() {
    const TOKENS_DECIMALS: u32 = 18;
    const BORROW_AMOUNT_ETH: u128 = 121 * 10u128.pow(TOKENS_DECIMALS); // 121 ETH
    const LIQUIDATOR_DEPOSIT_AMOUNT_ETH: u128 = 10_000 * 10u128.pow(TOKENS_DECIMALS); // 10_000 ETH
    const YEAR_IN_SECONDS: u64 = 31536000;

    // contract reserves: 1000 ETH
    // user deposited 200 ETH and 300 XLM
    // user borrowed 50 ETH
    let (env, lending_contract_client, admin, user, liquidator, token_xlm, token_eth) =
        success_borrow_setup();

    let mut ledger_info: LedgerInfo = env.ledger().get();
    ledger_info.timestamp = 10000;
    env.ledger().set(ledger_info.clone());

    let user_deposited_balance_eth: u128 =
        lending_contract_client.get_deposit(&user, &symbol_short!("eth"));

    assert_eq!(user_deposited_balance_eth, 200_000000000000000000); // 200 ETH

    let user_deposited_balance_xlm: u128 =
        lending_contract_client.get_deposit(&user, &symbol_short!("xlm"));

    assert_eq!(user_deposited_balance_xlm, 300_000000000000000000); // 300 XLM

    let user_collateral_usd: u128 = lending_contract_client.get_user_collateral_usd(&user);

    // 200 ETH * 2000 + 300 XLM * 10 == 403_000$
    assert_eq!(user_collateral_usd, 403_00000000000);

    let reserve_configuration_xlm: ReserveConfiguration =
        lending_contract_client.get_reserve_configuration(&symbol_short!("xlm"));

    assert_eq!(reserve_configuration_xlm.loan_to_value_ratio, 7500000); // ltv_xlm = 75%

    let reserve_configuration_eth: ReserveConfiguration =
        lending_contract_client.get_reserve_configuration(&symbol_short!("eth"));

    assert_eq!(reserve_configuration_eth.loan_to_value_ratio, 8500000); // ltv_eth = 85%

    let user_max_allowed_borrow_amount_usd: u128 =
        lending_contract_client.get_user_max_allowed_borrow_usd(&user);

    // 200 ETH * 0.85 * 2000 + 300 XLM * 0.75 * 10 == 340_000 + 2_250 = 342_250$
    assert_eq!(user_max_allowed_borrow_amount_usd, 342_250_00000000);

    let user_borrowed_usd: u128 = lending_contract_client.get_user_borrowed_usd(&user);

    assert_eq!(user_borrowed_usd, 100_000_00000000); // 50 ETH * 2000 = 100_000$

    let available_to_borrow_eth: u128 =
        lending_contract_client.get_available_to_borrow(&user, &symbol_short!("eth"));

    // (user_max_allowed_borrow_amount_usd - user_borrowed_usd) / price =
    // (342_250$ - 100_000$) / price = 242_250$ / price
    assert_eq!(available_to_borrow_eth, 121125000000000000000); // 242_250$ / 2000 == 121.125 ETH

    lending_contract_client.borrow(&user, &symbol_short!("eth"), &BORROW_AMOUNT_ETH);

    let available_to_borrow_eth: u128 =
        lending_contract_client.get_available_to_borrow(&user, &symbol_short!("eth"));

    assert_eq!(available_to_borrow_eth, 125000000000000000); // 0.125 ETH

    // TEST Full Borrow
    lending_contract_client.borrow(&user, &symbol_short!("eth"), &available_to_borrow_eth);
    let available_to_borrow_eth: u128 =
        lending_contract_client.get_available_to_borrow(&user, &symbol_short!("eth"));
    assert_eq!(available_to_borrow_eth, 0); // 0 ETH
}

#[test]
fn test_redeem() {
    // contract reserves: 1000 ETH and 1000 XLM
    // user deposited 200 ETH and 300 XLM
    let (env, lending_contract_client, admin, user) =
        success_deposit_as_collateral_of_diff_token_with_prices();

    let token_usdc = create_custom_token(&env, &admin, "USDC", "usdc", &7);
    let token_usdt = create_custom_token(&env, &admin, "USDT", "usdt", &7);

    token_usdc.mint(&admin, &100_000_0000000_i128);
    token_usdt.mint(&admin, &100_000_0000000_i128);

    env.budget().reset_unlimited();

    lending_contract_client.add_markets(
        &symbol_short!("usdc"),
        &token_usdc.address,
        &symbol_short!("USDC"),
        &7,
        &(75 * 10u128.pow(5)),
        &(80 * 10u128.pow(5)),
        &(5 * 10u128.pow(18)),
        &(30 * 10u128.pow(18)),
        &(70 * 10u128.pow(18)),
        &(80 * 10u128.pow(5)),
    );

    lending_contract_client.deposit(&admin, &symbol_short!("usdc"), &10_000_0000000);

    lending_contract_client.add_markets(
        &symbol_short!("usdt"),
        &token_usdt.address,
        &symbol_short!("USDT"),
        &7,
        &(75 * 10u128.pow(5)),
        &(80 * 10u128.pow(5)),
        &(5 * 10u128.pow(18)),
        &(30 * 10u128.pow(18)),
        &(70 * 10u128.pow(18)),
        &(80 * 10u128.pow(5)),
    );

    lending_contract_client.deposit(&admin, &symbol_short!("usdt"), &10_000_0000000);

    lending_contract_client.add_markets(
        &symbol_short!("dai"),
        &token_usdt.address,
        &symbol_short!("DAI"),
        &7,
        &(75 * 10u128.pow(5)),
        &(80 * 10u128.pow(5)),
        &(5 * 10u128.pow(18)),
        &(30 * 10u128.pow(18)),
        &(70 * 10u128.pow(18)),
        &(80 * 10u128.pow(5)),
    );

    lending_contract_client.deposit(&admin, &symbol_short!("dai"), &10_000_0000000);

    lending_contract_client.update_price(&symbol_short!("usdc"), &100_000_000);
    lending_contract_client.update_price(&symbol_short!("usdt"), &100_000_000);
    lending_contract_client.update_price(&symbol_short!("dai"), &100_000_000);

    token_usdc.mint(&user, &1000_0000000_i128);
    lending_contract_client.deposit(&user, &symbol_short!("usdc"), &1000_0000000);
    lending_contract_client.toggle_collateral_setting(&user, &symbol_short!("usdc"));
    env.budget().reset_unlimited();
    lending_contract_client.borrow(&user, &symbol_short!("xlm"), &100_000);
    println!("   Borrow 1: {:?}", env.budget().cpu_instruction_cost());
    // println!("{:?}", env.budget());
    env.budget().reset_unlimited();
    lending_contract_client.borrow(&user, &symbol_short!("eth"), &1_000);
    println!("   Borrow 1: {:?}", env.budget().cpu_instruction_cost());

    // println!("   Borrow test: {:?}", lending_contract_client.borrow_test(&user, &symbol_short!("eth"), &1_000));

    let user1 = Address::generate(&env);
    token_usdc.mint(&user1, &1000_0000000_i128);
    lending_contract_client.deposit(&user1, &symbol_short!("usdc"), &1000_0000000);
    lending_contract_client.toggle_collateral_setting(&user1, &symbol_short!("usdc"));
    env.budget().reset_unlimited();
    lending_contract_client.borrow(&user1, &symbol_short!("xlm"), &100_000);
    println!("   Borrow 2: {:?}", env.budget().cpu_instruction_cost());
    // println!("{:?}", env.budget());
    env.budget().reset_unlimited();
    lending_contract_client.borrow(&user1, &symbol_short!("eth"), &1_000);
    println!("   Borrow 2: {:?}", env.budget().cpu_instruction_cost());

    let user2 = Address::generate(&env);
    token_usdt.mint(&user2, &1000_0000000_i128);
    lending_contract_client.deposit(&user2, &symbol_short!("usdt"), &1000_0000000);
    lending_contract_client.toggle_collateral_setting(&user2, &symbol_short!("usdt"));
    env.budget().reset_unlimited();
    lending_contract_client.borrow(&user2, &symbol_short!("xlm"), &100_000);
    println!("   Borrow 3: {:?}", env.budget().cpu_instruction_cost());
    // println!("{:?}", env.budget());
    env.budget().reset_unlimited();
    lending_contract_client.borrow(&user2, &symbol_short!("eth"), &1_000);
    println!("   Borrow 3: {:?}", env.budget().cpu_instruction_cost());

    let user3 = Address::generate(&env);
    token_usdc.mint(&user3, &1000_0000000_i128);
    lending_contract_client.deposit(&user3, &symbol_short!("usdc"), &1000_0000000);
    lending_contract_client.toggle_collateral_setting(&user3, &symbol_short!("usdc"));
    env.budget().reset_unlimited();
    lending_contract_client.borrow(&user3, &symbol_short!("xlm"), &100_000);
    println!("   Borrow 4: {:?}", env.budget().cpu_instruction_cost());
    // println!("{:?}", env.budget());
    env.budget().reset_unlimited();
    lending_contract_client.borrow(&user3, &symbol_short!("eth"), &1_000);
    println!("   Borrow 4: {:?}", env.budget().cpu_instruction_cost());

    const TOKENS_DECIMALS: u32 = 18;
    const DEPOSIT_AMOUNT_ETH: u128 = 200 * 10u128.pow(TOKENS_DECIMALS);
    const DEPOSIT_AMOUNT_XLM: u128 = 300 * 10u128.pow(TOKENS_DECIMALS);

    env.budget().reset_unlimited();
    lending_contract_client.toggle_collateral_setting(&user, &symbol_short!("eth"));
    println!("CPU costs");
    println!(
        "      ToggleCollateralSetting : {:?}",
        env.budget().cpu_instruction_cost()
    );
    env.budget().reset_unlimited();
    lending_contract_client.toggle_collateral_setting(&user, &symbol_short!("eth"));
    println!(
        "      ToggleCollateralSetting : {:?}",
        env.budget().cpu_instruction_cost()
    );
    env.budget().reset_unlimited();
    let user_max_allowed_borrow_amount_usd: u128 =
        lending_contract_client.get_user_max_allowed_borrow_usd(&user);
    println!(
        "      GetUserMaxAllowedBorrowAmountUsd : {:?}",
        env.budget().cpu_instruction_cost()
    );
    env.budget().reset_unlimited();
    let available_to_redeem_eth: u128 =
        lending_contract_client.get_available_to_redeem(&user, &symbol_short!("eth"));
    println!(
        "      GetAvailableToRedeem eth : {:?}",
        env.budget().cpu_instruction_cost()
    );
    env.budget().reset_unlimited();
    let available_to_redeem_xlm: u128 =
        lending_contract_client.get_available_to_redeem(&user, &symbol_short!("xlm"));
    println!(
        "      GetAvailableToRedeem xlm: {:?}",
        env.budget().cpu_instruction_cost()
    );
    assert_eq!(available_to_redeem_eth, DEPOSIT_AMOUNT_ETH); // 200 ETH
    assert_eq!(available_to_redeem_xlm, DEPOSIT_AMOUNT_XLM); // 300 XLM

    env.budget().reset_unlimited();
    let available_to_borrow_eth: u128 =
        lending_contract_client.get_available_to_borrow(&user, &symbol_short!("eth"));
    println!(
        "      get_available_to_borrow eth: {:?}",
        env.budget().cpu_instruction_cost()
    );

    env.budget().reset_unlimited();
    lending_contract_client.get_user_collateral_usd(&user);
    println!(
        "           get_user_collateral_usd: {:?}",
        env.budget().cpu_instruction_cost()
    );
    env.budget().reset_unlimited();
    lending_contract_client.get_user_borrowed_usd(&user);
    println!(
        "             get_user_borrowed_usd: {:?}",
        env.budget().cpu_instruction_cost()
    );
    env.budget().reset_unlimited();
    lending_contract_client.get_user_liquidation_threshold(&user);
    println!(
        "    get_user_liquidation_threshold: {:?}",
        env.budget().cpu_instruction_cost()
    );
    env.budget().reset_unlimited();
    lending_contract_client.get_available_liquidity_by_token(&symbol_short!("eth"));
    println!(
        "   get_available_liquidity_by_token: {:?}",
        env.budget().cpu_instruction_cost()
    );
    env.budget().reset_unlimited();
    lending_contract_client.borrow(&user, &symbol_short!("eth"), &1_000_000);
    println!("   Borrow: {:?}", env.budget().cpu_instruction_cost());
    env.budget().reset_unlimited();
    lending_contract_client.redeem(&user, &symbol_short!("xlm"), &0);
    println!("   Redeem: {:?}", env.budget().cpu_instruction_cost());
    env.budget().reset_unlimited();
    lending_contract_client.redeem(&user, &symbol_short!("eth"), &0);
    let available_to_redeem_eth: u128 =
        lending_contract_client.get_available_to_redeem(&user, &symbol_short!("eth"));
    let available_to_redeem_xlm: u128 =
        lending_contract_client.get_available_to_redeem(&user, &symbol_short!("xlm"));
    assert_eq!(available_to_redeem_eth, 0); // 0 ETH
    assert_eq!(available_to_redeem_xlm, 0); // 0 XLM
}

#[test]
fn test_budget() {
    let env = Env::default();
    env.mock_all_auths();
    env.budget().reset_unlimited();
    let lending_contract_address = env.register_contract(None, LendingContract);
    // mod wasm_contract {
    //     soroban_sdk::contractimport!(file = "./target/wasm32-unknown-unknown/release/soroban_lending.wasm");
    // }
    // let lending_contract_address = &env.register_contract_wasm(None, wasm_contract::WASM);
    let lending_contract_client = LendingContractClient::new(&env, &lending_contract_address);
    let admin: Address = Address::generate(&env);
    let user = Address::generate(&env);
    let liquidator = Address::generate(&env);

    let token_xlm = create_custom_token(&env, &admin, "XLM", "xlm", &7);

    lending_contract_client.initialize(&admin, &liquidator);

    env.budget().reset_unlimited();
    lending_contract_client.add_markets(
        &symbol_short!("xlm"),
        &token_xlm.address,
        &symbol_short!("XLM"),
        &7,
        &(75 * 10u128.pow(5)),
        &(80 * 10u128.pow(5)),
        &(5 * 10u128.pow(18)),
        &(30 * 10u128.pow(18)),
        &(70 * 10u128.pow(18)),
        &(80 * 10u128.pow(5)),
    );
    println!("CPU costs");
    println!(
        "      AddMarkets: {:?}",
        env.budget().cpu_instruction_cost()
    );
    println!("{:?}", env.budget());
}

#[test]
fn test_tvl() {
    // contract reserves: 1000 ETH + 1000 XLM
    // user deposited 200 ETH and 300 XLM
    // 1200 * 2000 + 1300 * 10 = 2413000
    let (env, lending_contract_client, admin, user) = success_deposit_of_diff_token_with_prices();

    assert_eq!(lending_contract_client.get_tvl(), 2_413_000 * 10u128.pow(8)); // 2_313_000 USD
}
