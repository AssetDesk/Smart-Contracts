#![cfg(test)]

extern crate std;

use super::{LendingContract, LendingContractClient};
use crate::types::*;
use std::println;

use soroban_sdk::testutils::{Address as _, Events, Ledger, LedgerInfo};
use soroban_sdk::{symbol_short, token, Address, Env, IntoVal, String, Symbol};
use token::Client;

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
    const LTV_ATOM: u128 = 75 * 10u128.pow(PERCENT_DECIMALS); // 75%
    const LIQUIDATION_THRESHOLD_ATOM: u128 = 80 * 10u128.pow(PERCENT_DECIMALS); // 80%

    const INTEREST_RATE_DECIMALS: u32 = 18;
    const MIN_INTEREST_RATE: u128 = 5 * 10u128.pow(INTEREST_RATE_DECIMALS);
    const SAFE_BORROW_MAX_RATE: u128 = 30 * 10u128.pow(INTEREST_RATE_DECIMALS);
    const RATE_GROWTH_FACTOR: u128 = 70 * 10u128.pow(INTEREST_RATE_DECIMALS);

    const OPTIMAL_UTILIZATION_RATIO: u128 = 80 * 10u128.pow(PERCENT_DECIMALS);

    let env = Env::default();
    env.mock_all_auths();

    env.budget().reset_unlimited();
    let contract_address = env.register_contract(None, LendingContract);
    let contract_client = LendingContractClient::new(&env, &contract_address);
    let admin = Address::random(&env);
    let user1 = Address::random(&env);
    let liquidator = Address::random(&env);

    let token_atom = create_custom_token(&env, &admin, "Atom", "atom", &TOKENS_DECIMALS);
    let token_eth = create_custom_token(&env, &admin, "Eth", "eth", &TOKENS_DECIMALS);

    // token_atom.mint(&user1, &i128::try_from(INIT_USER_BALANCE).unwrap());
    token_eth.mint(&user1, &i128::try_from(INIT_USER_BALANCE).unwrap());

    // token_atom.mint(&admin, &i128::try_from(CONTRACT_RESERVES * 100).unwrap());
    token_eth.mint(&admin, &i128::try_from(CONTRACT_RESERVES).unwrap());

    token_eth.mint(
        &liquidator,
        &i128::try_from(INIT_LIQUIDATOR_BALANCE_ETH).unwrap(),
    );

    contract_client.AddMarkets(
        &symbol_short!("atom"),
        &token_atom.address,
        &symbol_short!("Atom"),
        &TOKENS_DECIMALS,
        &LTV_ATOM,
        &LIQUIDATION_THRESHOLD_ATOM,
        &MIN_INTEREST_RATE,
        &SAFE_BORROW_MAX_RATE,
        &RATE_GROWTH_FACTOR,
        &OPTIMAL_UTILIZATION_RATIO,
    );

    contract_client.AddMarkets(
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
    // token_atom.transfer(&admin, &contract_address, &i128::try_from(CONTRACT_RESERVES).unwrap());
    token_eth.transfer(
        &admin,
        &contract_address,
        &i128::try_from(CONTRACT_RESERVES).unwrap(),
    );

    // contract_client.UpdatePrice(&symbol_short!("atom"), &PRICE_ATOM);
    // contract_client.UpdatePrice(&symbol_short!("eth"), &PRICE_ETH);

    contract_client.deposit(&user1, &symbol_short!("eth"), &FIRST_DEPOSIT_AMOUNT_ETH);
    // contract_client.deposit(&admin, &symbol_short!("eth"), &(FIRST_DEPOSIT_AMOUNT_ETH * 15 / 10));

    // contract_client.ToggleCollateralSetting(&user1, &symbol_short!("eth"));
    // contract_client.ToggleCollateralSetting(&admin, &symbol_short!("eth"));

    let mut user_deposited_balance: u128 =
        contract_client.GetDeposit(&user1, &symbol_short!("eth"));

    assert_eq!(user_deposited_balance, FIRST_DEPOSIT_AMOUNT_ETH);
    assert_eq!(
        token_eth.balance(&user1),
        (INIT_USER_BALANCE - FIRST_DEPOSIT_AMOUNT_ETH) as i128
    );
    assert_eq!(
        token_eth.balance(&contract_address),
        (CONTRACT_RESERVES + FIRST_DEPOSIT_AMOUNT_ETH) as i128
    );

    contract_client.deposit(&user1, &symbol_short!("eth"), &SECOND_DEPOSIT_AMOUNT_ETH);
    // contract_client.Borrow(&admin, &symbol_short!("eth"), &(SECOND_DEPOSIT_AMOUNT_ETH / 2));

    user_deposited_balance = contract_client.GetDeposit(&user1, &symbol_short!("eth"));

    assert_eq!(
        user_deposited_balance,
        FIRST_DEPOSIT_AMOUNT_ETH + SECOND_DEPOSIT_AMOUNT_ETH
    );
    assert_eq!(
        token_eth.balance(&user1),
        (INIT_USER_BALANCE - FIRST_DEPOSIT_AMOUNT_ETH - SECOND_DEPOSIT_AMOUNT_ETH) as i128
    );
    assert_eq!(
        token_eth.balance(&contract_address),
        (CONTRACT_RESERVES + FIRST_DEPOSIT_AMOUNT_ETH + SECOND_DEPOSIT_AMOUNT_ETH) as i128
    );

    (contract_client, admin, user1)
}

pub fn success_deposit_of_diff_token_with_prices(
) -> (Env, LendingContractClient<'static>, Address, Address) {
    const TOKENS_DECIMALS: u32 = 18;

    const INIT_BALANCE_ETH: u128 = 1000 * 10u128.pow(TOKENS_DECIMALS);
    const INIT_BALANCE_ATOM: u128 = 1_000_000 * 10u128.pow(TOKENS_DECIMALS); // 1M ATOM

    const INIT_LIQUIDATOR_BALANCE_ETH: u128 = 1_000_000 * 10u128.pow(TOKENS_DECIMALS); // 1M ETH
    const INIT_LIQUIDATOR_BALANCE_ATOM: u128 = 1_000_000 * 10u128.pow(TOKENS_DECIMALS); // 1M ATOM

    const DEPOSIT_AMOUNT_ETH: u128 = 200 * 10u128.pow(TOKENS_DECIMALS);
    const DEPOSIT_AMOUNT_ATOM: u128 = 300 * 10u128.pow(TOKENS_DECIMALS);

    const CONTRACT_RESERVES_ETH: u128 = 1000 * 10u128.pow(TOKENS_DECIMALS);
    const CONTRACT_RESERVES_ATOM: u128 = 1000 * 10u128.pow(TOKENS_DECIMALS);

    const PERCENT_DECIMALS: u32 = 5;
    const LTV_ETH: u128 = 85 * 10u128.pow(PERCENT_DECIMALS); // 85%
    const LIQUIDATION_THRESHOLD_ETH: u128 = 90 * 10u128.pow(PERCENT_DECIMALS); // 90%
    const LTV_ATOM: u128 = 75 * 10u128.pow(PERCENT_DECIMALS); // 75%
    const LIQUIDATION_THRESHOLD_ATOM: u128 = 80 * 10u128.pow(PERCENT_DECIMALS); // 80%

    const INTEREST_RATE_DECIMALS: u32 = 18;
    const MIN_INTEREST_RATE: u128 = 5 * 10u128.pow(INTEREST_RATE_DECIMALS);
    const SAFE_BORROW_MAX_RATE: u128 = 30 * 10u128.pow(INTEREST_RATE_DECIMALS);
    const RATE_GROWTH_FACTOR: u128 = 70 * 10u128.pow(INTEREST_RATE_DECIMALS);

    const OPTIMAL_UTILIZATION_RATIO: u128 = 80 * 10u128.pow(PERCENT_DECIMALS);

    const PRICE_DECIMALS: u32 = 8;
    const PRICE_ETH: u128 = 2000 * 10u128.pow(PRICE_DECIMALS);
    const PRICE_ATOM: u128 = 10 * 10u128.pow(PRICE_DECIMALS);

    let env = Env::default();
    env.mock_all_auths();

    env.budget().reset_unlimited();
    let contract_address = env.register_contract(None, LendingContract);
    let contract_client = LendingContractClient::new(&env, &contract_address);
    let admin = Address::random(&env);
    let user1 = Address::random(&env);
    let liquidator = Address::random(&env);

    let token_atom = create_custom_token(&env, &admin, "Atom", "atom", &TOKENS_DECIMALS);
    let token_eth = create_custom_token(&env, &admin, "Eth", "eth", &TOKENS_DECIMALS);

    token_atom.mint(&user1, &i128::try_from(INIT_BALANCE_ATOM).unwrap());
    token_eth.mint(&user1, &i128::try_from(INIT_BALANCE_ETH).unwrap());

    token_atom.mint(&admin, &i128::try_from(INIT_BALANCE_ATOM).unwrap());
    token_eth.mint(&admin, &i128::try_from(INIT_BALANCE_ETH).unwrap());

    token_atom.mint(
        &liquidator,
        &i128::try_from(INIT_LIQUIDATOR_BALANCE_ATOM).unwrap(),
    );
    token_eth.mint(
        &liquidator,
        &i128::try_from(INIT_LIQUIDATOR_BALANCE_ETH).unwrap(),
    );

    contract_client.AddMarkets(
        &symbol_short!("atom"),
        &token_atom.address,
        &symbol_short!("Atom"),
        &TOKENS_DECIMALS,
        &LTV_ATOM,
        &LIQUIDATION_THRESHOLD_ATOM,
        &MIN_INTEREST_RATE,
        &SAFE_BORROW_MAX_RATE,
        &RATE_GROWTH_FACTOR,
        &OPTIMAL_UTILIZATION_RATIO,
    );

    contract_client.AddMarkets(
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
    token_atom.transfer(
        &admin,
        &contract_address,
        &i128::try_from(CONTRACT_RESERVES_ATOM).unwrap(),
    );
    token_eth.transfer(
        &admin,
        &contract_address,
        &i128::try_from(CONTRACT_RESERVES_ETH).unwrap(),
    );

    contract_client.UpdatePrice(&symbol_short!("atom"), &PRICE_ATOM);
    contract_client.UpdatePrice(&symbol_short!("eth"), &PRICE_ETH);

    let get_price_atom: u128 = contract_client.GetPrice(&symbol_short!("atom"));
    let get_price_eth: u128 = contract_client.GetPrice(&symbol_short!("eth"));

    assert_eq!(get_price_atom, 1000000000); // 10$
    assert_eq!(get_price_eth, 200000000000); // 2000$

    contract_client.deposit(&user1, &symbol_short!("eth"), &DEPOSIT_AMOUNT_ETH);

    let mut user_deposited_balance: u128 =
        contract_client.GetDeposit(&user1, &symbol_short!("eth"));

    assert_eq!(user_deposited_balance, DEPOSIT_AMOUNT_ETH);
    assert_eq!(
        token_eth.balance(&user1),
        (INIT_BALANCE_ETH - DEPOSIT_AMOUNT_ETH) as i128
    );
    assert_eq!(
        token_eth.balance(&contract_address),
        (CONTRACT_RESERVES_ETH + DEPOSIT_AMOUNT_ETH) as i128
    );

    contract_client.deposit(&user1, &symbol_short!("atom"), &DEPOSIT_AMOUNT_ATOM);

    user_deposited_balance = contract_client.GetDeposit(&user1, &symbol_short!("atom"));

    assert_eq!(user_deposited_balance, DEPOSIT_AMOUNT_ATOM);
    assert_eq!(
        token_atom.balance(&user1),
        (INIT_BALANCE_ATOM - DEPOSIT_AMOUNT_ATOM) as i128
    );
    assert_eq!(
        token_atom.balance(&contract_address),
        (CONTRACT_RESERVES_ATOM + DEPOSIT_AMOUNT_ATOM) as i128
    );

    (env, contract_client, admin, user1)
}

pub fn success_deposit_as_collateral_of_diff_token_with_prices(
) -> (Env, LendingContractClient<'static>, Address, Address) {
    let (env, contract_client, admin, user) = success_deposit_of_diff_token_with_prices();

    contract_client.ToggleCollateralSetting(&user, &symbol_short!("eth"));
    contract_client.ToggleCollateralSetting(&user, &symbol_short!("atom"));

    contract_client.ToggleCollateralSetting(&admin, &symbol_short!("eth"));
    contract_client.ToggleCollateralSetting(&admin, &symbol_short!("atom"));

    (env, contract_client, admin, user)
}

pub fn success_borrow_setup() -> (Env, LendingContractClient<'static>, Address, Address, token_contract::Client<'static>, token_contract::Client<'static>) {
    const TOKENS_DECIMALS: u32 = 18;

    const INIT_BALANCE_ETH: u128 = 10_000 * 10u128.pow(TOKENS_DECIMALS); // 10_000 ETH
    const INIT_BALANCE_ATOM: u128 = 10_000 * 10u128.pow(TOKENS_DECIMALS); // 10_000 ATOM
    const INIT_BALANCE_USDT: u128 = 10_000 * 10u128.pow(TOKENS_DECIMALS); // 10_000 USDT

    const INIT_LIQUIDATOR_BALANCE_ETH: u128 = 1_000_000 * 10u128.pow(TOKENS_DECIMALS); // 1M ETH
    const INIT_LIQUIDATOR_BALANCE_ATOM: u128 = 1_000_000 * 10u128.pow(TOKENS_DECIMALS); // 1M ATOM

    const DEPOSIT_AMOUNT_ETH: u128 = 200 * 10u128.pow(TOKENS_DECIMALS);
    const DEPOSIT_AMOUNT_ATOM: u128 = 300 * 10u128.pow(TOKENS_DECIMALS);

    const CONTRACT_RESERVES_ETH: u128 = 1000 * 10u128.pow(TOKENS_DECIMALS);
    const CONTRACT_RESERVES_ATOM: u128 = 1000 * 10u128.pow(TOKENS_DECIMALS);

    const BORROW_AMOUNT_ETH: u128 = 50 * 10u128.pow(TOKENS_DECIMALS);

    const PERCENT_DECIMALS: u32 = 5;
    const LTV_ETH: u128 = 85 * 10u128.pow(PERCENT_DECIMALS); // 85%
    const LIQUIDATION_THRESHOLD_ETH: u128 = 90 * 10u128.pow(PERCENT_DECIMALS); // 90%
    const LTV_ATOM: u128 = 75 * 10u128.pow(PERCENT_DECIMALS); // 75%
    const LIQUIDATION_THRESHOLD_ATOM: u128 = 80 * 10u128.pow(PERCENT_DECIMALS); // 80%

    const INTEREST_RATE_DECIMALS: u32 = 18;
    const MIN_INTEREST_RATE: u128 = 5 * 10u128.pow(INTEREST_RATE_DECIMALS);
    const SAFE_BORROW_MAX_RATE: u128 = 30 * 10u128.pow(INTEREST_RATE_DECIMALS);
    const RATE_GROWTH_FACTOR: u128 = 70 * 10u128.pow(INTEREST_RATE_DECIMALS);

    const OPTIMAL_UTILIZATION_RATIO: u128 = 80 * 10u128.pow(PERCENT_DECIMALS);

    const PRICE_DECIMALS: u32 = 8;
    const PRICE_ETH: u128 = 2000 * 10u128.pow(PRICE_DECIMALS);
    const PRICE_ATOM: u128 = 10 * 10u128.pow(PRICE_DECIMALS);

    
    let env = Env::default();
    env.mock_all_auths();

    env.budget().reset_unlimited();
    let contract_address = env.register_contract(None, LendingContract);
    let contract_client = LendingContractClient::new(&env, &contract_address);
    let admin = Address::random(&env);
    let user1 = Address::random(&env);
    let liquidator = Address::random(&env);

    let token_atom = create_custom_token(&env, &admin, "Atom", "atom", &TOKENS_DECIMALS);
    let token_eth = create_custom_token(&env, &admin, "Eth", "eth", &TOKENS_DECIMALS);
    let token_usdt = create_custom_token(&env, &admin, "USDT", "usdt", &TOKENS_DECIMALS);

    token_atom.mint(&user1, &i128::try_from(INIT_BALANCE_ATOM).unwrap());
    token_eth.mint(&user1, &i128::try_from(INIT_BALANCE_ETH).unwrap());
    token_usdt.mint(&user1, &i128::try_from(INIT_BALANCE_ETH).unwrap());

    token_atom.mint(&admin, &i128::try_from(CONTRACT_RESERVES_ETH).unwrap());
    token_eth.mint(&admin, &i128::try_from(CONTRACT_RESERVES_ETH).unwrap());

    token_atom.mint(
        &liquidator,
        &i128::try_from(INIT_LIQUIDATOR_BALANCE_ATOM).unwrap(),
    );
    token_eth.mint(
        &liquidator,
        &i128::try_from(INIT_LIQUIDATOR_BALANCE_ETH).unwrap(),
    );

    contract_client.AddMarkets(
        &symbol_short!("atom"),
        &token_atom.address,
        &symbol_short!("Atom"),
        &TOKENS_DECIMALS,
        &LTV_ATOM,
        &LIQUIDATION_THRESHOLD_ATOM,
        &MIN_INTEREST_RATE,
        &SAFE_BORROW_MAX_RATE,
        &RATE_GROWTH_FACTOR,
        &OPTIMAL_UTILIZATION_RATIO,
    );

    contract_client.AddMarkets(
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
    token_atom.transfer(
        &admin,
        &contract_address,
        &i128::try_from(CONTRACT_RESERVES_ATOM).unwrap(),
    );
    token_eth.transfer(
        &admin,
        &contract_address,
        &i128::try_from(CONTRACT_RESERVES_ETH).unwrap(),
    );

    contract_client.UpdatePrice(&symbol_short!("atom"), &PRICE_ATOM);
    contract_client.UpdatePrice(&symbol_short!("eth"), &PRICE_ETH);

    let get_price_atom: u128 = contract_client.GetPrice(&symbol_short!("atom"));
    let get_price_eth: u128 = contract_client.GetPrice(&symbol_short!("eth"));

    assert_eq!(get_price_atom, 1000000000); // 10$
    assert_eq!(get_price_eth, 200000000000); // 2000$


    contract_client.ToggleCollateralSetting(&user1, &symbol_short!("eth"));
    contract_client.ToggleCollateralSetting(&user1, &symbol_short!("atom"));

    contract_client.ToggleCollateralSetting(&admin, &symbol_short!("eth"));
    contract_client.ToggleCollateralSetting(&admin, &symbol_short!("atom"));


    contract_client.deposit(&user1, &symbol_short!("eth"), &DEPOSIT_AMOUNT_ETH);

    let current_info: LedgerInfo = env.ledger().get();

    env.ledger().set(LedgerInfo {
        timestamp: 1000,
        protocol_version: current_info.protocol_version,
        sequence_number: current_info.sequence_number,
        network_id: current_info.network_id,
        base_reserve: current_info.base_reserve,
        min_temp_entry_expiration: current_info.min_temp_entry_expiration,
        min_persistent_entry_expiration: current_info.min_persistent_entry_expiration,
        max_entry_expiration: current_info.max_entry_expiration,
    });


    let _available_to_redeem: u128 = contract_client.GetAvailableToRedeem(&user1, &symbol_short!("eth"));

    let user_deposited_balance: u128 = contract_client.GetDeposit(&user1, &symbol_short!("eth"));
    

    assert_eq!(user_deposited_balance, DEPOSIT_AMOUNT_ETH);

    assert_eq!(
        token_eth.balance(&user1) as u128,
        INIT_BALANCE_ETH - DEPOSIT_AMOUNT_ETH
    );

    assert_eq!(
        token_eth.balance(&contract_address) as u128,
        CONTRACT_RESERVES_ETH + DEPOSIT_AMOUNT_ETH
    );

    contract_client.deposit(&user1, &symbol_short!("atom"), &DEPOSIT_AMOUNT_ATOM);

    env.ledger().set(LedgerInfo {
        timestamp: 2000,
        protocol_version: current_info.protocol_version,
        sequence_number: current_info.sequence_number,
        network_id: current_info.network_id,
        base_reserve: current_info.base_reserve,
        min_temp_entry_expiration: current_info.min_temp_entry_expiration,
        min_persistent_entry_expiration: current_info.min_persistent_entry_expiration,
        max_entry_expiration: current_info.max_entry_expiration,
    });

    let user_deposited_balance: u128 = contract_client.GetDeposit(&user1, &symbol_short!("atom"));

    assert_eq!(user_deposited_balance, DEPOSIT_AMOUNT_ATOM);

    assert_eq!(
        token_atom.balance(&user1) as u128,
        INIT_BALANCE_ATOM - DEPOSIT_AMOUNT_ATOM
    );

    assert_eq!(
        token_atom.balance(&contract_address) as u128,
        CONTRACT_RESERVES_ATOM + DEPOSIT_AMOUNT_ATOM
    );

    env.ledger().set(LedgerInfo {
        timestamp: 10000,
        protocol_version: current_info.protocol_version,
        sequence_number: current_info.sequence_number,
        network_id: current_info.network_id,
        base_reserve: current_info.base_reserve,
        min_temp_entry_expiration: current_info.min_temp_entry_expiration,
        min_persistent_entry_expiration: current_info.min_persistent_entry_expiration,
        max_entry_expiration: current_info.max_entry_expiration,
    });

    contract_client.Borrow(
        &user1,
        &symbol_short!("eth"),
        &BORROW_AMOUNT_ETH,
    );

    (env, contract_client, admin, user1, token_atom, token_eth)
}

#[test]
fn test_successful_deposits_of_one_token() {
    const TOKENS_DECIMALS: u32 = 18;

    const INIT_USER_BALANCE: u128 = 1000 * 10u128.pow(TOKENS_DECIMALS);
    const INIT_LIQUIDATOR_BALANCE_ETH: u128 = 1_000_000 * 10u128.pow(TOKENS_DECIMALS); // 1M ETH
    const INIT_LIQUIDATOR_BALANCE_ATOM: u128 = 1_000_000 * 10u128.pow(TOKENS_DECIMALS); // 1M ATOM

    const CONTRACT_RESERVES: u128 = 1_000_000 * 10u128.pow(TOKENS_DECIMALS);
    const FIRST_DEPOSIT_AMOUNT: u128 = 200 * 10u128.pow(TOKENS_DECIMALS);
    const SECOND_DEPOSIT_AMOUNT: u128 = 300 * 10u128.pow(TOKENS_DECIMALS);

    const PERCENT_DECIMALS: u32 = 5;
    const LTV_ETH: u128 = 85 * 10u128.pow(PERCENT_DECIMALS); // 85%
    const LIQUIDATION_THRESHOLD_ETH: u128 = 90 * 10u128.pow(PERCENT_DECIMALS); // 90%
    const LTV_ATOM: u128 = 75 * 10u128.pow(PERCENT_DECIMALS); // 75%
    const LIQUIDATION_THRESHOLD_ATOM: u128 = 80 * 10u128.pow(PERCENT_DECIMALS); // 80%

    const INTEREST_RATE_DECIMALS: u32 = 18;
    const MIN_INTEREST_RATE: u128 = 5 * 10u128.pow(INTEREST_RATE_DECIMALS);
    const SAFE_BORROW_MAX_RATE: u128 = 30 * 10u128.pow(INTEREST_RATE_DECIMALS);
    const RATE_GROWTH_FACTOR: u128 = 70 * 10u128.pow(INTEREST_RATE_DECIMALS);

    const OPTIMAL_UTILIZATION_RATIO: u128 = 80 * 10u128.pow(PERCENT_DECIMALS);

    const PRICE_DECIMALS: u32 = 8;
    const PRICE_ETH: u128 = 2000 * 10u128.pow(PRICE_DECIMALS);
    const PRICE_ATOM: u128 = 10 * 10u128.pow(PRICE_DECIMALS);

    let env = Env::default();
    env.mock_all_auths();

    env.budget().reset_unlimited();
    let contract_address = env.register_contract(None, LendingContract);
    let contract_client = LendingContractClient::new(&env, &contract_address);
    let admin = Address::random(&env);
    let user1 = Address::random(&env);

    let token_atom = create_custom_token(&env, &admin, "Atom", "atom", &TOKENS_DECIMALS);
    let token_eth = create_custom_token(&env, &admin, "Eth", "eth", &TOKENS_DECIMALS);

    token_atom.mint(&user1, &i128::try_from(INIT_USER_BALANCE).unwrap());
    token_eth.mint(&user1, &i128::try_from(INIT_USER_BALANCE).unwrap());

    token_atom.mint(&admin, &i128::try_from(CONTRACT_RESERVES * 100).unwrap());
    token_eth.mint(&admin, &i128::try_from(CONTRACT_RESERVES * 100).unwrap());

    contract_client.AddMarkets(
        &symbol_short!("atom"),
        &token_atom.address,
        &symbol_short!("Atom"),
        &TOKENS_DECIMALS,
        &LTV_ATOM,
        &LIQUIDATION_THRESHOLD_ATOM,
        &5000000000000000000,
        &20000000000000000000,
        &100000000000000000000,
        &OPTIMAL_UTILIZATION_RATIO,
    );

    contract_client.AddMarkets(
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
    token_atom.transfer(
        &admin,
        &contract_address,
        &i128::try_from(CONTRACT_RESERVES / 100).unwrap(),
    );
    token_eth.transfer(
        &admin,
        &contract_address,
        &i128::try_from(CONTRACT_RESERVES / 100).unwrap(),
    );

    contract_client.UpdatePrice(&symbol_short!("atom"), &PRICE_ATOM);
    contract_client.UpdatePrice(&symbol_short!("eth"), &PRICE_ETH);

    contract_client.deposit(&user1, &symbol_short!("eth"), &FIRST_DEPOSIT_AMOUNT);
    contract_client.deposit(
        &admin,
        &symbol_short!("eth"),
        &(FIRST_DEPOSIT_AMOUNT * 15 / 10),
    );

    contract_client.ToggleCollateralSetting(&user1, &symbol_short!("eth"));
    contract_client.ToggleCollateralSetting(&admin, &symbol_short!("eth"));

    let mut user_deposited_balance: u128 =
        contract_client.GetDeposit(&user1, &symbol_short!("eth"));

    assert_eq!(user_deposited_balance, FIRST_DEPOSIT_AMOUNT);
    assert_eq!(
        token_eth.balance(&user1),
        (INIT_USER_BALANCE - FIRST_DEPOSIT_AMOUNT) as i128
    );

    contract_client.deposit(&user1, &symbol_short!("eth"), &SECOND_DEPOSIT_AMOUNT);
    contract_client.Borrow(&admin, &symbol_short!("eth"), &(SECOND_DEPOSIT_AMOUNT / 2));

    let current_info: LedgerInfo = env.ledger().get();

    println!("Current ledger info: {:?}", current_info);

    env.ledger().set(LedgerInfo {
        timestamp: 31536000,
        protocol_version: current_info.protocol_version,
        sequence_number: current_info.sequence_number,
        network_id: current_info.network_id,
        base_reserve: current_info.base_reserve,
        min_temp_entry_expiration: current_info.min_temp_entry_expiration,
        min_persistent_entry_expiration: current_info.min_persistent_entry_expiration,
        max_entry_expiration: current_info.max_entry_expiration,
    });

    println!("New timestamp: {:?}", env.ledger().timestamp());

    let total_borrow_data: TotalBorrowData =
        contract_client.GetTotalBorrowData(&symbol_short!("eth"));
    println!("Total borrow data: {:?}", total_borrow_data);

    let reserves_by_token: u128 = contract_client.GetTotalReservesByToken(&symbol_short!("eth"));
    println!("Total Reserves for Eth : {:?}", reserves_by_token);

    user_deposited_balance = contract_client.GetDeposit(&user1, &symbol_short!("eth"));
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
    let (contract_client, admin, user) = success_deposit_of_one_token_setup();

    let user_deposit_amount_eth: u128 = contract_client.GetDeposit(&user, &symbol_short!("eth"));
    let user_deposit_amount_atom: u128 = contract_client.GetDeposit(&user, &symbol_short!("atom"));

    assert_eq!(user_deposit_amount_atom, 0); // 0
    assert_eq!(user_deposit_amount_eth, 500000000000000000000); // 500
}

#[test]
fn test_get_mm_token_price() {
    let (contract_client, admin, user) = success_deposit_of_one_token_setup();

    let get_mm_token_price_eth: u128 = contract_client.GetMmTokenPrice(&symbol_short!("eth"));
    let get_mm_token_price_atom: u128 = contract_client.GetMmTokenPrice(&symbol_short!("atom"));

    assert_eq!(get_mm_token_price_atom, 1000000000000000000); // 1:1
    assert_eq!(get_mm_token_price_eth, 1000000000000000000); // 1:1
}

#[test]
fn test_get_liquidity_rate() {
    // contract reserves: 1000 ETH and 1000 ATOM
    // user deposited 200 ETH and 300 ATOM
    let (env, contract_client, admin, user) =
        success_deposit_as_collateral_of_diff_token_with_prices();

    const DECIMAL_FRACTIONAL: u128 = 1_000_000_000_000_000_000_u128; // 1*10**18
    const BORROW_SECOND_TOKEN_FIRST_PART: u128 = 300 * DECIMAL_FRACTIONAL;

    contract_client.Borrow(
        &user,
        &symbol_short!("atom"),
        &BORROW_SECOND_TOKEN_FIRST_PART,
    );

    let get_liquidity_rate_eth: u128 = contract_client.GetLiquidityRate(&symbol_short!("eth"));
    let get_liquidity_rate_atom: u128 = contract_client.GetLiquidityRate(&symbol_short!("atom"));

    assert_eq!(get_liquidity_rate_atom, 1153846153846153846); // ~1.154%
    assert_eq!(get_liquidity_rate_eth, 0);
}

#[test]
fn test_get_user_borrow_amount_with_interest() {
    const TOKENS_DECIMALS: u32 = 18;
    const BORROW_AMOUNT_ETH: u128 = 50 * 10u128.pow(TOKENS_DECIMALS); // 50 ETH
    const BORROW_AMOUNT_ATOM: u128 = 200 * 10u128.pow(TOKENS_DECIMALS); // 200 ATOM

    const YEAR_IN_SECONDS: u64 = 31536000;

    // contract reserves: 1000 ETH and 1000 ATOM
    // user deposited 200 ETH and 300 ATOM
    let (env, contract_client, admin, user) =
        success_deposit_as_collateral_of_diff_token_with_prices();

    let mut user_borrow_amount_with_interest_eth: u128 =
        contract_client.GetUserBorrowAmountWithInterest(&user, &symbol_short!("eth"));

    let mut user_borrow_amount_with_interest_atom: u128 =
        contract_client.GetUserBorrowAmountWithInterest(&user, &symbol_short!("atom"));

    // user hasn't borrowed anything yet
    assert_eq!(user_borrow_amount_with_interest_eth, 0);
    assert_eq!(user_borrow_amount_with_interest_atom, 0);

    contract_client.Borrow(&user, &symbol_short!("eth"), &BORROW_AMOUNT_ETH);
    contract_client.Borrow(&user, &symbol_short!("atom"), &BORROW_AMOUNT_ATOM);

    user_borrow_amount_with_interest_eth =
        contract_client.GetUserBorrowAmountWithInterest(&user, &symbol_short!("eth"));

    user_borrow_amount_with_interest_atom =
        contract_client.GetUserBorrowAmountWithInterest(&user, &symbol_short!("atom"));

    assert_eq!(user_borrow_amount_with_interest_eth, 50000000000000000000); // 50 ETH
    assert_eq!(user_borrow_amount_with_interest_atom, 200000000000000000000); // 200 ATOM

    let current_info: LedgerInfo = env.ledger().get();

    env.ledger().set(LedgerInfo {
        timestamp: YEAR_IN_SECONDS,
        protocol_version: current_info.protocol_version,
        sequence_number: current_info.sequence_number,
        network_id: current_info.network_id,
        base_reserve: current_info.base_reserve,
        min_temp_entry_expiration: current_info.min_temp_entry_expiration,
        min_persistent_entry_expiration: current_info.min_persistent_entry_expiration,
        max_entry_expiration: current_info.max_entry_expiration,
    });

    user_borrow_amount_with_interest_eth =
        contract_client.GetUserBorrowAmountWithInterest(&user, &symbol_short!("eth"));

    user_borrow_amount_with_interest_atom =
        contract_client.GetUserBorrowAmountWithInterest(&user, &symbol_short!("atom"));

    // 50 ETH + 5% borrow APY = 50 ETH + 2.5 ETH = 52.5 ETH
    assert_eq!(user_borrow_amount_with_interest_eth, 52500000000000000000);
    // 200 ATOM + 5% borrow APY = 200 ATOM + 10 ATOM = 210 ATOM
    assert_eq!(user_borrow_amount_with_interest_atom, 210000000000000000000);

    // let users_with_borrow = contract_client.GetAllUsersWithBorrows();

    // assert!(!users_with_borrow.is_empty());
}

#[test]
fn test_success_borrow_one_token() {
    const DECIMAL_FRACTIONAL: u128 = 1_000000_000000_000000_u128; // 1*10**18

    const INIT_BALANCE_SECOND_TOKEN: u128 = 1_000_000 * DECIMAL_FRACTIONAL; // 1M ATOM

    const DEPOSIT_OF_SECOND_TOKEN: u128 = 300 * DECIMAL_FRACTIONAL;

    const BORROW_SECOND_TOKEN: u128 = 300 * DECIMAL_FRACTIONAL;

    /*
    price eth 1500
    price atom 10

    deposited eth 200 * 1500 = 300_000 $

    borrowed atom 300 * 10 = 3_000 $
    */

    // contract reserves: 1000 ETH and 1000 ATOM
    // user deposited 200 ETH and 300 ATOM
    let (env, contract_client, admin, user) =
        success_deposit_as_collateral_of_diff_token_with_prices();

    contract_client.Redeem(&user, &symbol_short!("atom"), &DEPOSIT_OF_SECOND_TOKEN);


    let user_deposited_balance_after_redeeming: u128 = contract_client.GetDeposit(&user, &symbol_short!("atom"));

    assert_eq!(user_deposited_balance_after_redeeming, 0);

    // assert_eq!(
    //     app.wrap()
    //         .query_balance("user", "atom")
    //         .unwrap()
    //         .amount
    //         .u128(),
    //     INIT_BALANCE_SECOND_TOKEN
    // );


    contract_client.Borrow(&user, &symbol_short!("atom"), &BORROW_SECOND_TOKEN);

    let current_info: LedgerInfo = env.ledger().get();

    env.ledger().set(LedgerInfo {
        timestamp: 31536000,
        protocol_version: current_info.protocol_version,
        sequence_number: current_info.sequence_number,
        network_id: current_info.network_id,
        base_reserve: current_info.base_reserve,
        min_temp_entry_expiration: current_info.min_temp_entry_expiration,
        min_persistent_entry_expiration: current_info.min_persistent_entry_expiration,
        max_entry_expiration: current_info.max_entry_expiration,
    });

    let user_borrowed_balance: u128 = contract_client.GetUserBorrowAmountWithInterest(&user, &symbol_short!("atom"));

    assert_ne!(user_borrowed_balance, BORROW_SECOND_TOKEN);
    assert_eq!(
        user_borrowed_balance,
        BORROW_SECOND_TOKEN * 105 / 100
    );
}

#[test]
fn test_success_repay_whole_amount() {
    // user borrowed 50 ETH
    let (env, contract_client, admin, user, token_atom, token_eth) = success_borrow_setup();

    let current_info: LedgerInfo = env.ledger().get();

    env.ledger().set(LedgerInfo {
        timestamp: 3153600 + 10000,
        protocol_version: current_info.protocol_version,
        sequence_number: current_info.sequence_number,
        network_id: current_info.network_id,
        base_reserve: current_info.base_reserve,
        min_temp_entry_expiration: current_info.min_temp_entry_expiration,
        min_persistent_entry_expiration: current_info.min_persistent_entry_expiration,
        max_entry_expiration: current_info.max_entry_expiration,
    });


    let user_borrow_amount_with_interest: u128 = contract_client.GetUserBorrowAmountWithInterest(&user, &symbol_short!("eth"));

    let amount_to_repay_with_interest = user_borrow_amount_with_interest;

    contract_client.Repay(&user, &symbol_short!("eth"), &amount_to_repay_with_interest);

    let user_borrow_amount_with_interest: u128 = contract_client.GetUserBorrowAmountWithInterest(&user, &symbol_short!("eth"));

    assert_eq!(user_borrow_amount_with_interest, 0);
}

#[test]
fn test_success_repay_more_than_needed() {
    // user borrowed 50 ETH
    let (env, contract_client, admin, user, token_atom, token_eth) = success_borrow_setup();

    let mut ledger_info: LedgerInfo = env.ledger().get();
    ledger_info.timestamp = 3153600 + 10000;
    env.ledger().set(ledger_info);

    let user_borrow_amount_with_interest: u128 = contract_client.GetUserBorrowAmountWithInterest(&user, &symbol_short!("eth"));

    let amount_to_repay_with_interest: u128 = user_borrow_amount_with_interest;

    let underlying_balance_before_repay: i128 = token_eth.balance(&contract_client.address);

    contract_client.Repay(&user, &symbol_short!("eth"), &(amount_to_repay_with_interest * 2));

    let underlying_balance_after_repay: i128 = token_eth.balance(&contract_client.address);

    // paying only what we supposed to, not twice as much
    assert_eq!(
        underlying_balance_after_repay - amount_to_repay_with_interest as i128,
        underlying_balance_before_repay
    );

    let user_borrow_amount_with_interest: u128 = contract_client.GetUserBorrowAmountWithInterest(&user, &symbol_short!("eth"));

    assert_eq!(user_borrow_amount_with_interest, 0);
}

#[test]
fn test_decimal() {
    let env = Env::default();
    env.mock_all_auths();

    env.budget().reset_unlimited();
    let contract_address = env.register_contract(None, LendingContract);
    let contract_client = LendingContractClient::new(&env, &contract_address);
    let admin: Address = Address::random(&env);

    let decimals: u32 = 10;
    let u128_max: u128 = u128::MAX / 10_u128.pow(decimals.clone());

    let (num1, num2) = contract_client.test_decimal(&u128_max, &18);
    
    println!("Numbers: {:?}, {:?}", num1, num2);
    println!("Number : {:?}", num1 / 10_u128.pow(18));
    assert_eq!(num1, num2);
}
