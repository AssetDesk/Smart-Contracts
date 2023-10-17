#![cfg(test)]

extern crate std;

use super::{LendingContract, LendingContractClient};
use crate::types::*;
use std::println;

use soroban_sdk::testutils::{Address as _, Events, Ledger, LedgerInfo};
use soroban_sdk::{symbol_short, token, Address, Env, IntoVal, String, Symbol};
use token::Client as TokenClient;
use token::StellarAssetClient as TokenAdminClient;

mod token_contract {
    soroban_sdk::contractimport!(
        file =
            "./token/soroban_token_contract.optimized.wasm"
    );
}

fn create_custom_token<'a>(e: &Env, admin: &Address, name: &str, symbol: &str, decimals: &u32) -> token_contract::Client<'a> {
    let token_id = &e.register_contract_wasm(None, token_contract::WASM);
    let token = token_contract::Client::new(e, &token_id);
    token.initialize(admin, decimals, &String::from_slice(&e, name), &String::from_slice(&e, symbol));
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

    token_eth.mint(&liquidator, &i128::try_from(INIT_LIQUIDATOR_BALANCE_ETH).unwrap());

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
    token_eth.transfer(&admin, &contract_address, &i128::try_from(CONTRACT_RESERVES).unwrap());

    // contract_client.UpdatePrice(&symbol_short!("atom"), &PRICE_ATOM);
    // contract_client.UpdatePrice(&symbol_short!("eth"), &PRICE_ETH);

    contract_client.deposit(&user1, &symbol_short!("eth"), &FIRST_DEPOSIT_AMOUNT_ETH);
    // contract_client.deposit(&admin, &symbol_short!("eth"), &(FIRST_DEPOSIT_AMOUNT_ETH * 15 / 10));

    // contract_client.ToggleCollateralSetting(&user1, &symbol_short!("eth"));
    // contract_client.ToggleCollateralSetting(&admin, &symbol_short!("eth"));

    let mut user_deposited_balance: u128 = contract_client.GetDeposit(&user1, &symbol_short!("eth"));

    assert_eq!(user_deposited_balance, FIRST_DEPOSIT_AMOUNT_ETH);
    assert_eq!(token_eth.balance(&user1), (INIT_USER_BALANCE - FIRST_DEPOSIT_AMOUNT_ETH) as i128);
    assert_eq!(token_eth.balance(&contract_address), (CONTRACT_RESERVES + FIRST_DEPOSIT_AMOUNT_ETH) as i128);

    contract_client.deposit(&user1, &symbol_short!("eth"), &SECOND_DEPOSIT_AMOUNT_ETH);
    // contract_client.Borrow(&admin, &symbol_short!("eth"), &(SECOND_DEPOSIT_AMOUNT_ETH / 2));

    user_deposited_balance = contract_client.GetDeposit(&user1, &symbol_short!("eth"));

    assert_eq!(user_deposited_balance, FIRST_DEPOSIT_AMOUNT_ETH + SECOND_DEPOSIT_AMOUNT_ETH);
    assert_eq!(token_eth.balance(&user1), (INIT_USER_BALANCE - FIRST_DEPOSIT_AMOUNT_ETH - SECOND_DEPOSIT_AMOUNT_ETH) as i128);
    assert_eq!(token_eth.balance(&contract_address), (CONTRACT_RESERVES + FIRST_DEPOSIT_AMOUNT_ETH + SECOND_DEPOSIT_AMOUNT_ETH) as i128);

    (contract_client, admin, user1)
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
    token_atom.transfer(&admin, &contract_address, &i128::try_from(CONTRACT_RESERVES / 100).unwrap());
    token_eth.transfer(&admin, &contract_address, &i128::try_from(CONTRACT_RESERVES / 100).unwrap());

    contract_client.UpdatePrice(&symbol_short!("atom"), &PRICE_ATOM);
    contract_client.UpdatePrice(&symbol_short!("eth"), &PRICE_ETH);

    contract_client.deposit(&user1, &symbol_short!("eth"), &FIRST_DEPOSIT_AMOUNT);
    contract_client.deposit(&admin, &symbol_short!("eth"), &(FIRST_DEPOSIT_AMOUNT * 15 / 10));

    contract_client.ToggleCollateralSetting(&user1, &symbol_short!("eth"));
    contract_client.ToggleCollateralSetting(&admin, &symbol_short!("eth"));

    let mut user_deposited_balance: u128 = contract_client.GetDeposit(&user1, &symbol_short!("eth"));

    assert_eq!(user_deposited_balance, FIRST_DEPOSIT_AMOUNT);
    assert_eq!(token_eth.balance(&user1), (INIT_USER_BALANCE - FIRST_DEPOSIT_AMOUNT) as i128);

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

    let total_borrow_data: TotalBorrowData = contract_client.GetTotalBorrowData(&symbol_short!("eth"));
    println!("Total borrow data: {:?}", total_borrow_data);

    let reserves_by_token: u128 = contract_client.GetTotalReservesByToken(&symbol_short!("eth"));
    println!("Total Reserves for Eth : {:?}", reserves_by_token);

    user_deposited_balance = contract_client.GetDeposit(&user1, &symbol_short!("eth"));
    println!("User initial deposit       : {:?}", FIRST_DEPOSIT_AMOUNT + SECOND_DEPOSIT_AMOUNT);
    println!("User deposit after set time: {:?}", user_deposited_balance);
    assert!(user_deposited_balance > FIRST_DEPOSIT_AMOUNT + SECOND_DEPOSIT_AMOUNT);
    assert_eq!(token_eth.balance(&user1), (INIT_USER_BALANCE - FIRST_DEPOSIT_AMOUNT - SECOND_DEPOSIT_AMOUNT) as i128);

}


#[test]
fn test_get_deposit() {
    // having 500 deposited we want to redeem SECOND_DEPOSIT_AMOUNT
    // so that FIRST_DEPOSIT_AMOUNT is remaining
    let (contract_client, admin, user) = success_deposit_of_one_token_setup();

    let mut user_deposit_amount_eth: u128 = contract_client.GetDeposit(&user, &symbol_short!("eth"));
    let mut user_deposit_amount_atom: u128 = contract_client.GetDeposit(&user, &symbol_short!("atom"));

    assert_eq!(user_deposit_amount_atom, 0); // 0
    assert_eq!(
        user_deposit_amount_eth,
        500000000000000000000
    ); // 500
}

#[test]
    fn test_get_mm_token_price() {
        // having 500 deposited we want to redeem SECOND_DEPOSIT_AMOUNT
        // so that FIRST_DEPOSIT_AMOUNT is remaining
        let (contract_client, admin, user) = success_deposit_of_one_token_setup();

        let get_mm_token_price_eth: u128 = contract_client.GetMmTokenPrice( &symbol_short!("eth"));

        let get_mm_token_price_atom: u128 = contract_client.GetMmTokenPrice( &symbol_short!("atom"));

        assert_eq!(get_mm_token_price_atom, 1000000000000000000); // 1:1
        assert_eq!(get_mm_token_price_eth, 1000000000000000000); // 1:1
    }