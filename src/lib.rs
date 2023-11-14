#![no_std]
#![allow(non_snake_case)]

use crate::types::{
    DataKey, LiquidityIndexData, ReserveConfiguration, TokenInfo, TokenInterestRateModelParams,
    TotalBorrowData, UserBorrowingInfo, MONTH_BUMP_AMOUNT, MONTH_LIFETIME_THRESHOLD,
};

use soroban_sdk::{contract, contractimpl, token, Address, Env, Symbol, Vec, symbol_short, Map, map}; // contracterror, panic_with_error, vec

use core::ops::{Add, Div, Mul};
use rust_decimal::prelude::{Decimal, MathematicalOps, ToPrimitive};

mod types;

const PERCENT_DECIMALS: u32 = 5;
const HUNDRED_PERCENT: u128 = 100 * 10u128.pow(PERCENT_DECIMALS);

const INTEREST_RATE_DECIMALS: u32 = 18;
const INTEREST_RATE_MULTIPLIER: u128 = 10u128.pow(INTEREST_RATE_DECIMALS);
const HUNDRED: u128 = 100;
const YEAR_IN_SECONDS: u128 = 31536000; // 365 days

const USD_DECIMALS: u32 = 8;

pub trait DecimalExt {
    fn to_u128_with_decimals(&self, decimals: u32) -> Result<u128, rust_decimal::Error>;
}

// impl DecimalExt for Decimal {
//     // converting high-precise numbers into u128
//     fn to_u128_with_decimals(&self, decimals: u32) -> Result<u128, rust_decimal::Error> {
//         let s = self.to_string();
//         let (left, right) = s.split_once(".").unwrap_or((&s, ""));
//         let mut right = right.to_string();
//         let right_len = right.len() as u32;
//         if right_len > decimals {
//             right.truncate(decimals.try_into().unwrap());
//         } else if right_len < decimals {
//             let zeroes = decimals - right_len;
//             right.push_str(&"0".repeat(zeroes.try_into().unwrap()));
//         }
//         let s = format!("{}{}", left, right);
//         Ok(s.parse::<u128>().unwrap_or(0))
//     }
// }

impl DecimalExt for Decimal {
    // converting high-precise numbers into u128
    fn to_u128_with_decimals(&self, decimals: u32) -> Result<u128, rust_decimal::Error> {
        let number_dec_new: Decimal = self * Decimal::new(10_i64.pow(decimals), 0);
        Ok(number_dec_new.to_u128().unwrap_or(0))
    }
}

fn has_administrator(e: &Env) -> bool {
    let key = DataKey::Admin;
    e.storage().persistent().has(&key)
}

fn read_administrator(e: &Env) -> Address {
    let key = DataKey::Admin;
    e.storage().persistent().get(&key).unwrap()
}

fn write_administrator(e: &Env, admin: &Address) {
    let key = DataKey::Admin;
    e.storage().persistent().set(&key, admin);
    e.storage()
        .persistent()
        .bump(&key, MONTH_LIFETIME_THRESHOLD, MONTH_BUMP_AMOUNT);
}

fn read_liquidator(e: &Env) -> Address {
    let key = DataKey::Liquidator;
    e.storage().persistent().get(&key).unwrap()
}

fn write_liquidator(e: &Env, liquidator: &Address) {
    let key = DataKey::Liquidator;
    e.storage().persistent().set(&key, liquidator);
    e.storage()
        .persistent()
        .bump(&key, MONTH_LIFETIME_THRESHOLD, MONTH_BUMP_AMOUNT);
}

fn get_deposit(env: Env, user: Address, denom: Symbol) -> u128 {
    // calculates user deposit including deposit interest
    let token_decimals = get_token_decimal(env.clone(), denom.clone());

    let user_mm_token_balance: u128 = env
        .storage()
        .persistent()
        .get(&DataKey::USER_MM_TOKEN_BALANCE(user.clone(), denom.clone()))
        .unwrap_or(0_u128);

    let mm_token_price = get_mm_token_price(env.clone(), denom.clone());

    let user_token_balance =
        Decimal::from_i128_with_scale(user_mm_token_balance as i128, token_decimals)
            .mul(Decimal::from_i128_with_scale(
                mm_token_price as i128,
                token_decimals,
            ))
            .to_u128_with_decimals(token_decimals)
            .unwrap();

    user_token_balance
}

fn get_available_liquidity_by_token(env: Env, denom: Symbol) -> u128 {
    let contract_address = env.current_contract_address();
    let token_info: Map<Symbol, TokenInfo> = env
        .storage()
        .persistent()
        .get(&DataKey::SUPPORTED_TOKENS)
        .unwrap_or(Map::new(&env));
    token_balance(&env, &token_info.get(denom).unwrap().address, &contract_address) as u128
}

fn get_total_borrow_data(env: Env, denom: Symbol) -> TotalBorrowData {
    let total_borrow_data: Map<Symbol, TotalBorrowData> = env
        .storage()
        .persistent()
        .get(&DataKey::TOTAL_BORROW_DATA)
        .unwrap_or(Map::new(&env));
    total_borrow_data.get(denom).unwrap()
}

fn get_interest_rate(env: Env, denom: Symbol) -> u128 {
    let utilization_rate = get_utilization_rate_by_token(env.clone(), denom.clone());

    let token_interest: TokenInterestRateModelParams = env
        .storage()
        .persistent()
        .get(&DataKey::TOKENS_INTEREST_RATE_MODEL_PARAM(denom.clone()))
        .unwrap();

    let min_interest_rate: u128 = token_interest.min_interest_rate;
    let safe_borrow_max_rate: u128 = token_interest.safe_borrow_max_rate;
    let rate_growth_factor: u128 = token_interest.rate_growth_factor;
    let optimal_utilization_ratio: u128 = token_interest.optimal_utilization_ratio;

    if utilization_rate <= optimal_utilization_ratio {
        min_interest_rate
            + utilization_rate * (safe_borrow_max_rate - min_interest_rate)
                / optimal_utilization_ratio
    } else {
        safe_borrow_max_rate
            + rate_growth_factor * (utilization_rate - optimal_utilization_ratio)
                / (HUNDRED_PERCENT - optimal_utilization_ratio)
    }
}

fn get_token_decimal(env: Env, denom: Symbol) -> u32 {
    let token_info: Map<Symbol, TokenInfo> = env
        .storage()
        .persistent()
        .get(&DataKey::SUPPORTED_TOKENS)
        .unwrap();
    token_info.get(denom).unwrap().decimals
}

fn get_token_address(env: Env, denom: Symbol) -> Address {
    let token_info: Map<Symbol, TokenInfo> = env
        .storage()
        .persistent()
        .get(&DataKey::SUPPORTED_TOKENS)
        .unwrap();
    token_info.get(denom).unwrap().address
}

fn get_supported_tokens(env: Env) -> Vec<Symbol> {
    env.storage()
        .persistent()
        .get(&DataKey::SUPPORTED_TOKENS_LIST)
        .unwrap_or(Vec::<Symbol>::new(&env))
}

fn get_total_borrowed_by_token(env: Env, denom: Symbol) -> u128 {
    let total_borrow_data: TotalBorrowData = get_total_borrow_data(env.clone(), denom.clone());

    let token_decimals: u32 = get_token_decimal(env.clone(), denom.clone());

    let total_borrowed_amount_with_interest: u128 = calc_borrow_amount_with_interest(
        total_borrow_data.total_borrowed_amount,
        total_borrow_data.average_interest_rate,
        (env.ledger().timestamp() - total_borrow_data.timestamp) as u128,
        token_decimals,
    );

    total_borrowed_amount_with_interest
}

fn get_user_max_allowed_borrow_amount_usd(env: Env, user: Address) -> u128 {
    // the maximum amount in USD that a user can borrow
    let mut max_allowed_borrow_amount_usd = 0u128;

    for token in get_supported_tokens(env.clone()) {
        let use_user_deposit_as_collateral =
            user_deposit_as_collateral(env.clone(), user.clone(), token.clone());

        if use_user_deposit_as_collateral {
            let user_deposit: u128 = get_deposit(env.clone(), user.clone(), token.clone());

            let reserve_configuration: ReserveConfiguration = env
                .storage()
                .persistent()
                .get(&DataKey::RESERVE_CONFIGURATION(token.clone()))
                .unwrap();

            let loan_to_value_ratio: u128 = reserve_configuration.loan_to_value_ratio;

            let token_decimals: u32 = get_token_decimal(env.clone(), token.clone());

            let price: u128 = fetch_price_by_token(env.clone(), token.clone());

            let user_deposit_usd: u128 =
                Decimal::from_i128_with_scale(user_deposit as i128, token_decimals)
                    .mul(Decimal::from_i128_with_scale(price as i128, USD_DECIMALS))
                    .to_u128_with_decimals(USD_DECIMALS)
                    .unwrap();

            max_allowed_borrow_amount_usd +=
                user_deposit_usd * loan_to_value_ratio / HUNDRED_PERCENT;
        }
    }

    max_allowed_borrow_amount_usd
}

fn get_utilization_rate_by_token(env: Env, denom: Symbol) -> u128 {
    let reserves_by_token = get_total_reserves_by_token(env.clone(), denom.clone());

    if reserves_by_token != 0 {
        let borrowed_by_token = get_total_borrowed_by_token(env, denom.clone());

        borrowed_by_token * HUNDRED_PERCENT / reserves_by_token
    } else {
        0_u128
    }
}

fn get_reserve_configuration(env: Env, denom: Symbol) -> ReserveConfiguration {
    let reserve_configuration: ReserveConfiguration = env
        .storage()
        .persistent()
        .get(&DataKey::RESERVE_CONFIGURATION(denom.clone()))
        .unwrap();
    reserve_configuration
}

fn get_user_utilization_rate(env: Env, user: Address) -> u128 {
    let sum_collateral_balance_usd: u128 = get_user_collateral_usd(env.clone(), user.clone());

    if sum_collateral_balance_usd != 0 {
        let sum_user_borrow_balance_usd: u128 = get_user_borrowed_usd(env.clone(), user.clone());

        sum_user_borrow_balance_usd * HUNDRED_PERCENT / sum_collateral_balance_usd
    } else {
        0_u128
    }
}

fn calc_borrow_amount_with_interest(
    borrowed_amount: u128,
    interest_rate: u128,
    interval: u128,
    token_decimals: u32,
) -> u128 {
    let base = Decimal::from_i128_with_scale(
        (interest_rate / HUNDRED + INTEREST_RATE_MULTIPLIER) as i128,
        INTEREST_RATE_DECIMALS,
    );

    let exponent = Decimal::from_i128_with_scale(
        (interval * INTEREST_RATE_MULTIPLIER / YEAR_IN_SECONDS) as i128,
        INTEREST_RATE_DECIMALS,
    );

    let borrow_amount_with_interest: u128 =
        Decimal::from_i128_with_scale(borrowed_amount as i128, token_decimals)
            .mul(base.powd(exponent))
            .to_u128_with_decimals(token_decimals)
            .unwrap();

    borrow_amount_with_interest
}

fn get_user_borrowing_info(env: Env, user: Address, denom: Symbol) -> UserBorrowingInfo {
    let user_borrowing_info: UserBorrowingInfo = env
        .storage()
        .persistent()
        .get(&DataKey::USER_BORROWING_INFO(user.clone(), denom.clone()))
        .unwrap_or_default();

    let mut average_interest_rate: u128 = user_borrowing_info.average_interest_rate;
    let mut timestamp: u64 = user_borrowing_info.timestamp;
    if user_borrowing_info.borrowed_amount == 0_u128 {
        let current_interest_rate = get_interest_rate(env.clone(), denom.clone());

        average_interest_rate = current_interest_rate;
        timestamp = env.ledger().timestamp();
    }

    UserBorrowingInfo {
        borrowed_amount: user_borrowing_info.borrowed_amount,
        average_interest_rate: average_interest_rate,
        timestamp: timestamp,
    }
}

fn get_user_borrow_amount_with_interest(env: Env, user: Address, denom: Symbol) -> u128 {
    let current_borrowing_info = get_user_borrowing_info(env.clone(), user.clone(), denom.clone());

    let token_decimals = get_token_decimal(env.clone(), denom.clone());

    let borrow_amount_with_interest = calc_borrow_amount_with_interest(
        current_borrowing_info.borrowed_amount,
        current_borrowing_info.average_interest_rate,
        (env.ledger().timestamp() - current_borrowing_info.timestamp) as u128,
        token_decimals,
    );

    borrow_amount_with_interest
}

fn get_total_reserves_by_token(env: Env, denom: Symbol) -> u128 {
    let token_liquidity: u128 = get_available_liquidity_by_token(env.clone(), denom.clone());
    let borrowed_by_token: u128 = get_total_borrowed_by_token(env.clone(), denom.clone());
    token_liquidity + borrowed_by_token
}

fn get_liquidity_rate(env: Env, denom: Symbol) -> u128 {
    let total_borrow_data: TotalBorrowData = get_total_borrow_data(env.clone(), denom.clone());
    let expected_annual_interest_income: u128 = total_borrow_data.expected_annual_interest_income;

    let reserves_by_token: u128 = get_total_reserves_by_token(env.clone(), denom.clone());

    let token_decimals: u32 = get_token_decimal(env.clone(), denom.clone());

    if reserves_by_token == 0 {
        0u128
    } else {
        let liquidity_rate: u128 = Decimal::from_i128_with_scale(
            expected_annual_interest_income as i128,
            INTEREST_RATE_DECIMALS,
        )
        .mul(Decimal::from_i128_with_scale(HUNDRED as i128, 0u32))
        .div(Decimal::from_i128_with_scale(
            reserves_by_token as i128,
            token_decimals,
        ))
        .to_u128_with_decimals(INTEREST_RATE_DECIMALS)
        .unwrap();

        liquidity_rate
    }
}

fn get_current_liquidity_index_ln(env: Env, denom: Symbol) -> u128 {
    let liquidity_rate: u128 = get_liquidity_rate(env.clone(), denom.clone());
    let liquidity_index_data: LiquidityIndexData = env
        .storage()
        .persistent()
        .get(&DataKey::LIQUIDITY_INDEX_DATA)
        .unwrap_or(Map::new(&env))
        .get(denom.clone())
        .unwrap();

    let liquidity_index_last_update: u64 = liquidity_index_data.timestamp;

    let liquidity_index_ln: u128 = liquidity_index_data.liquidity_index_ln;

    let new_liquidity_index_ln: u128 = ((env.ledger().timestamp())
        .checked_sub(liquidity_index_last_update)
        .unwrap_or_default()) as u128
        * Decimal::from_i128_with_scale(
            (liquidity_rate / HUNDRED + INTEREST_RATE_MULTIPLIER) as i128,
            INTEREST_RATE_DECIMALS,
        )
        .ln()
        .to_u128_with_decimals(INTEREST_RATE_DECIMALS)
        .unwrap()
        / YEAR_IN_SECONDS
        + liquidity_index_ln;

    new_liquidity_index_ln
}

fn execute_update_liquidity_index_data(env: Env, denom: Symbol) {
    let current_liquidity_index_ln = get_current_liquidity_index_ln(env.clone(), denom.clone());

    let new_liquidity_index_data = LiquidityIndexData {
        denom: denom.clone(),
        liquidity_index_ln: current_liquidity_index_ln,
        timestamp: env.ledger().timestamp(),
    };

    let mut liquidity_map: Map<Symbol, LiquidityIndexData> = env.storage().persistent().get(
        &DataKey::LIQUIDITY_INDEX_DATA).unwrap_or(Map::new(&env));
    liquidity_map.set(denom.clone(), new_liquidity_index_data);
    env.storage().persistent().set(
        &DataKey::LIQUIDITY_INDEX_DATA,
        &liquidity_map,
    );
    env.storage().persistent().bump(
        &DataKey::LIQUIDITY_INDEX_DATA,
        MONTH_LIFETIME_THRESHOLD,
        MONTH_BUMP_AMOUNT,
    );
}

fn get_mm_token_price(env: Env, denom: Symbol) -> u128 {
    // number of tokens that correspond to one mmToken
    let token_decimals: u32 = get_token_decimal(env.clone(), denom.clone());

    let current_liquidity_index_ln: u128 =
        get_current_liquidity_index_ln(env.clone(), denom.clone());

    let mm_token_price =
        Decimal::from_i128_with_scale(current_liquidity_index_ln as i128, INTEREST_RATE_DECIMALS)
            .exp()
            .to_u128_with_decimals(token_decimals)
            .unwrap_or_default();

    mm_token_price
}

fn user_deposit_as_collateral(env: Env, user: Address, denom: Symbol) -> bool {
    let use_user_deposit_as_collateral: bool = env
        .storage()
        .persistent()
        .get(&DataKey::USER_DEPOSIT_AS_COLLATERAL(
            user.clone(),
            denom.clone(),
        ))
        .unwrap_or(false);

    // // POC: Only xlm is used as a collateral
    // let mut use_user_deposit_as_collateral: bool = false;
    // if denom == symbol_short!("xlm") {
    //     use_user_deposit_as_collateral = true;
    // }

    use_user_deposit_as_collateral
}

fn fetch_price_by_token(env: Env, denom: Symbol) -> u128 {
    env.storage()
        .persistent()
        .get(&DataKey::PRICES(denom.clone()))
        .unwrap_or(0_u128)
}

fn get_user_deposited_usd(env: Env, user: Address) -> u128 {
    let mut user_deposited_usd = 0u128;

    for token in get_supported_tokens(env.clone()) {
        let user_deposit: u128 = get_deposit(env.clone(), user.clone(), token.clone());

        let token_decimals: u32 = get_token_decimal(env.clone(), token.clone());

        let price = fetch_price_by_token(env.clone(), token.clone());

        user_deposited_usd += Decimal::from_i128_with_scale(user_deposit as i128, token_decimals)
            .mul(Decimal::from_i128_with_scale(price as i128, USD_DECIMALS))
            .to_u128_with_decimals(USD_DECIMALS)
            .unwrap();
    }

    user_deposited_usd
}

fn get_user_collateral_usd(env: Env, user: Address) -> u128 {
    let mut user_collateral_usd = 0_u128;

    for token in get_supported_tokens(env.clone()) {
        let use_user_deposit_as_collateral =
            user_deposit_as_collateral(env.clone(), user.clone(), token.clone());

        if use_user_deposit_as_collateral {
            let user_deposit = get_deposit(env.clone(), user.clone(), token.clone());

            let token_decimals = get_token_decimal(env.clone(), token.clone());

            let price = fetch_price_by_token(env.clone(), token.clone());

            user_collateral_usd +=
                Decimal::from_i128_with_scale(user_deposit as i128, token_decimals)
                    .mul(Decimal::from_i128_with_scale(price as i128, USD_DECIMALS))
                    .to_u128_with_decimals(USD_DECIMALS)
                    .unwrap()
        }
    }

    user_collateral_usd
}

fn get_user_borrowed_usd(env: Env, user: Address) -> u128 {
    let mut user_borrowed_usd: u128 = 0_u128;
    for token in get_supported_tokens(env.clone()) {
        let user_borrow_amount_with_interest =
            get_user_borrow_amount_with_interest(env.clone(), user.clone(), token.clone());

        let token_decimals = get_token_decimal(env.clone(), token.clone());

        let price = fetch_price_by_token(env.clone(), token.clone());

        user_borrowed_usd +=
            Decimal::from_i128_with_scale(user_borrow_amount_with_interest as i128, token_decimals)
                .mul(Decimal::from_i128_with_scale(price as i128, USD_DECIMALS))
                .to_u128_with_decimals(USD_DECIMALS)
                .unwrap()
    }

    user_borrowed_usd
}

fn get_available_to_borrow(env: Env, user: Address, denom: Symbol) -> u128 {
    let mut available_to_borrow = 0u128;

    // maximum amount allowed for borrowing
    let max_allowed_borrow_amount_usd =
        get_user_max_allowed_borrow_amount_usd(env.clone(), user.clone());

    let sum_user_borrow_balance_usd = get_user_borrowed_usd(env.clone(), user.clone());

    if max_allowed_borrow_amount_usd > sum_user_borrow_balance_usd {
        let token_decimals = get_token_decimal(env.clone(), denom.clone());

        let price = fetch_price_by_token(env.clone(), denom.clone());

        available_to_borrow = Decimal::from_i128_with_scale(
            (max_allowed_borrow_amount_usd - sum_user_borrow_balance_usd) as i128,
            USD_DECIMALS,
        )
        .div(Decimal::from_i128_with_scale(price as i128, USD_DECIMALS))
        .to_u128_with_decimals(token_decimals)
        .unwrap();

        let token_liquidity = get_available_liquidity_by_token(env.clone(), denom.clone());

        if available_to_borrow > token_liquidity {
            available_to_borrow = token_liquidity
        }
    }

    available_to_borrow
}

fn get_available_to_redeem(env: Env, user: Address, denom: Symbol) -> u128 {
    let mut available_to_redeem: u128 = 0u128;

    let user_token_balance: u128 = get_deposit(env.clone(), user.clone(), denom.clone());

    if user_deposit_as_collateral(env.clone(), user.clone(), denom.clone()) {
        if user_token_balance != 0 {
            let sum_collateral_balance_usd: u128 =
                get_user_collateral_usd(env.clone(), user.clone());
            let sum_borrow_balance_usd: u128 = get_user_borrowed_usd(env.clone(), user.clone());

            let user_liquidation_threshold =
                get_user_liquidation_threshold(env.clone(), user.clone());

            let required_collateral_balance_usd =
                sum_borrow_balance_usd * HUNDRED_PERCENT / user_liquidation_threshold;

            let token_liquidity: u128 =
                get_available_liquidity_by_token(env.clone(), denom.clone());

            if sum_collateral_balance_usd >= required_collateral_balance_usd {
                let token_decimals: u32 = get_token_decimal(env.clone(), denom.clone());

                let price: u128 = fetch_price_by_token(env.clone(), denom.clone());

                available_to_redeem = Decimal::from_i128_with_scale(
                    (sum_collateral_balance_usd - required_collateral_balance_usd) as i128,
                    USD_DECIMALS,
                )
                .div(Decimal::from_i128_with_scale(price as i128, USD_DECIMALS))
                .to_u128_with_decimals(token_decimals)
                .unwrap();

                if available_to_redeem > user_token_balance {
                    available_to_redeem = user_token_balance;
                }

                if available_to_redeem > token_liquidity {
                    available_to_redeem = token_liquidity;
                }
            }
        }
    } else {
        available_to_redeem = user_token_balance;
    }

    available_to_redeem
}

fn get_user_liquidation_threshold(env: Env, user: Address) -> u128 {
    // the minimum borrowing amount in USD, upon reaching which the user's loan positions are liquidated
    let mut liquidation_threshold_borrow_amount_usd = 0u128;
    let mut user_collateral_usd = 0u128;

    for token in get_supported_tokens(env.clone()) {
        let use_user_deposit_as_collateral =
            user_deposit_as_collateral(env.clone(), user.clone(), token.clone());

        if use_user_deposit_as_collateral {
            let user_deposit: u128 = get_deposit(env.clone(), user.clone(), token.clone());

            let reserve_configuration: ReserveConfiguration = env
                .storage()
                .persistent()
                .get(&DataKey::RESERVE_CONFIGURATION(token.clone()))
                .unwrap();
            let liquidation_threshold = reserve_configuration.liquidation_threshold;

            let token_decimals = get_token_decimal(env.clone(), token.clone());

            let price = fetch_price_by_token(env.clone(), token.clone());

            let user_deposit_usd =
                Decimal::from_i128_with_scale(user_deposit as i128, token_decimals)
                    .mul(Decimal::from_i128_with_scale(price as i128, USD_DECIMALS))
                    .to_u128_with_decimals(USD_DECIMALS)
                    .unwrap();

            liquidation_threshold_borrow_amount_usd +=
                user_deposit_usd * liquidation_threshold / HUNDRED_PERCENT;
            user_collateral_usd += user_deposit_usd;
        }
    }

    liquidation_threshold_borrow_amount_usd * HUNDRED_PERCENT / user_collateral_usd
}

fn move_token(env: &Env, token: &Address, from: &Address, to: &Address, transfer_amount: i128) {
    // new token interface
    let token_client = token::Client::new(&env, &token);
    token_client.transfer(&from, to, &transfer_amount);
}

fn token_balance(env: &Env, token: &Address, user_address: &Address) -> i128 {
    let token_client = token::Client::new(&env, &token);
    token_client.balance(&user_address)
}

#[contract]
pub struct LendingContract;

#[contractimpl]
impl LendingContract {
    pub fn initialize(e: Env, admin: Address, liquidator: Address) {
        if has_administrator(&e) {
            panic!("already initialized")
        }
        write_administrator(&e, &admin);
        write_liquidator(&e, &liquidator);
    }

    pub fn Deposit(env: Env, user_address: Address, denom: Symbol, deposited_token_amount: u128) {
        user_address.require_auth();

        let token_address: Address = get_token_address(env.clone(), denom.clone());
        move_token(
            &env,
            &token_address,
            &user_address,
            &env.current_contract_address(),
            deposited_token_amount.clone() as i128,
        );

        execute_update_liquidity_index_data(env.clone(), denom.clone());

        let token_decimals: u32 = get_token_decimal(env.clone(), denom.clone());
        let mm_token_price: u128 = get_mm_token_price(env.clone(), denom.clone());

        let deposited_mm_token_amount =
            Decimal::from_i128_with_scale(deposited_token_amount as i128, token_decimals)
                .div(Decimal::from_i128_with_scale(
                    mm_token_price as i128,
                    token_decimals,
                ))
                .to_u128_with_decimals(token_decimals)
                .unwrap();

        let user_current_mm_token_balance: u128 = env
            .storage()
            .persistent()
            .get(&DataKey::USER_MM_TOKEN_BALANCE(
                user_address.clone(),
                denom.clone(),
            ))
            .unwrap_or(0_u128);

        let new_user_mm_token_balance: u128 =
            user_current_mm_token_balance + deposited_mm_token_amount;

        env.storage().persistent().set(
            &DataKey::USER_MM_TOKEN_BALANCE(user_address.clone(), denom.clone()),
            &new_user_mm_token_balance,
        );
        env.storage().persistent().bump(
            &DataKey::USER_MM_TOKEN_BALANCE(user_address.clone(), denom.clone()),
            MONTH_LIFETIME_THRESHOLD,
            MONTH_BUMP_AMOUNT,
        );
    }

    pub fn AddMarkets(
        env: Env,
        denom: Symbol,
        address: Address,
        name: Symbol,
        decimals: u32,
        loan_to_value_ratio: u128,
        liquidation_threshold: u128,
        min_interest_rate: u128,
        safe_borrow_max_rate: u128,
        rate_growth_factor: u128,
        optimal_utilization_ratio: u128,
    ) {
        // Admin only
        let admin: Address = read_administrator(&env);
        admin.require_auth();

        let mut supported_tokens: Vec<Symbol> = get_supported_tokens(env.clone());
        if supported_tokens.contains(denom.clone()) {
            panic!("There already exists such a supported token");
        }

        supported_tokens.push_back(denom.clone());
        env.storage()
            .persistent()
            .set(&DataKey::SUPPORTED_TOKENS_LIST, &supported_tokens);
        env.storage().persistent().bump(
            &DataKey::SUPPORTED_TOKENS_LIST,
            MONTH_LIFETIME_THRESHOLD,
            MONTH_BUMP_AMOUNT,
        );

        let token_info: TokenInfo = TokenInfo {
            denom: denom.clone(),
            address,
            name,
            symbol: denom.clone(),
            decimals,
        };

        let mut supported_tokens_info: Map<Symbol, TokenInfo> = env
            .storage()
            .persistent()
            .get(&DataKey::SUPPORTED_TOKENS)
            .unwrap_or(Map::new(&env));
        supported_tokens_info.set(denom.clone(), token_info);
        env.storage()
            .persistent()
            .set(&DataKey::SUPPORTED_TOKENS, &supported_tokens_info);
        env.storage().persistent().bump(
            &DataKey::SUPPORTED_TOKENS,
            MONTH_LIFETIME_THRESHOLD,
            MONTH_BUMP_AMOUNT,
        );

        let reserve_configuration: ReserveConfiguration = ReserveConfiguration {
            denom: denom.clone(),
            loan_to_value_ratio,
            liquidation_threshold,
        };
        env.storage().persistent().set(
            &DataKey::RESERVE_CONFIGURATION(denom.clone()),
            &reserve_configuration,
        );
        env.storage().persistent().bump(
            &DataKey::RESERVE_CONFIGURATION(denom.clone()),
            MONTH_LIFETIME_THRESHOLD,
            MONTH_BUMP_AMOUNT,
        );

        let tokens_interest_rate_model_params: TokenInterestRateModelParams =
            TokenInterestRateModelParams {
                denom: denom.clone(),
                min_interest_rate,
                safe_borrow_max_rate,
                rate_growth_factor,
                optimal_utilization_ratio,
            };
        env.storage().persistent().set(
            &DataKey::TOKENS_INTEREST_RATE_MODEL_PARAM(denom.clone()),
            &tokens_interest_rate_model_params,
        );
        env.storage().persistent().bump(
            &DataKey::TOKENS_INTEREST_RATE_MODEL_PARAM(denom.clone()),
            MONTH_LIFETIME_THRESHOLD,
            MONTH_BUMP_AMOUNT,
        );

        let total_borrow_data: TotalBorrowData = TotalBorrowData {
            denom: denom.clone(),
            total_borrowed_amount: 0_u128,
            expected_annual_interest_income: 0_u128,
            average_interest_rate: 0_u128,
            timestamp: env.ledger().timestamp(),
        };

        let mut total_borrow_map: Map<Symbol, TotalBorrowData> = env.storage().persistent().get(
            &DataKey::TOTAL_BORROW_DATA).unwrap_or(Map::new(&env));
        total_borrow_map.set(denom.clone(), total_borrow_data);
        env.storage().persistent().set(
            &DataKey::TOTAL_BORROW_DATA,
            &total_borrow_map,
        );
        env.storage().persistent().bump(
            &DataKey::TOTAL_BORROW_DATA,
            MONTH_LIFETIME_THRESHOLD,
            MONTH_BUMP_AMOUNT,
        );

        let liquidity_index_data: LiquidityIndexData = LiquidityIndexData {
            denom: denom.clone(),
            liquidity_index_ln: 0_u128,
            timestamp: env.ledger().timestamp(),
        };

        let mut liquidity_map: Map<Symbol, LiquidityIndexData> = env.storage().persistent().get(
        &DataKey::LIQUIDITY_INDEX_DATA).unwrap_or(Map::new(&env));
        liquidity_map.set(denom.clone(), liquidity_index_data);
        env.storage().persistent().set(
            &DataKey::LIQUIDITY_INDEX_DATA,
            &liquidity_map,
        );
        env.storage().persistent().bump(
            &DataKey::LIQUIDITY_INDEX_DATA,
            MONTH_LIFETIME_THRESHOLD,
            MONTH_BUMP_AMOUNT,
        );
    }

    pub fn UpdatePrice(env: Env, denom: Symbol, price: u128) {
        // Admin only
        let admin: Address = read_administrator(&env);
        admin.require_auth();

        env.storage()
            .persistent()
            .set(&DataKey::PRICES(denom.clone()), &price);
        env.storage().persistent().bump(
            &DataKey::PRICES(denom.clone()),
            MONTH_LIFETIME_THRESHOLD,
            MONTH_BUMP_AMOUNT,
        );
    }

    pub fn ToggleCollateralSetting(env: Env, user: Address, denom: Symbol) {
        user.require_auth();

        let use_user_deposit_as_collateral =
            user_deposit_as_collateral(env.clone(), user.clone(), denom.clone());

        if use_user_deposit_as_collateral {
            let user_token_balance: u128 = get_deposit(env.clone(), user.clone(), denom.clone());

            if user_token_balance != 0 {
                let token_decimals = get_token_decimal(env.clone(), denom.clone());

                let price = fetch_price_by_token(env.clone(), denom.clone());

                let user_token_balance_usd =
                    Decimal::from_i128_with_scale(user_token_balance as i128, token_decimals)
                        .mul(Decimal::from_i128_with_scale(price as i128, USD_DECIMALS))
                        .to_u128_with_decimals(USD_DECIMALS)
                        .unwrap();

                let sum_collateral_balance_usd = get_user_collateral_usd(env.clone(), user.clone());

                let sum_borrow_balance_usd = get_user_borrowed_usd(env.clone(), user.clone());

                let user_liquidation_threshold =
                    get_user_liquidation_threshold(env.clone(), user.clone());

                assert!(
                    sum_borrow_balance_usd * HUNDRED_PERCENT / user_liquidation_threshold < sum_collateral_balance_usd - user_token_balance_usd,
                    "The collateral has already using to collateralise the borrowing. Not enough available balance"
                );
            }
        }

        env.storage().persistent().set(
            &DataKey::USER_DEPOSIT_AS_COLLATERAL(user.clone(), denom.clone()),
            &!use_user_deposit_as_collateral,
        );
        env.storage().persistent().bump(
            &DataKey::USER_DEPOSIT_AS_COLLATERAL(user.clone(), denom.clone()),
            MONTH_LIFETIME_THRESHOLD,
            MONTH_BUMP_AMOUNT,
        );
    }

    pub fn Borrow(env: Env, user: Address, denom: Symbol, amount: u128) {
        user.require_auth();

        // let liquidator = read_liquidator(&env);

        // if user == liquidator {
        //     panic!("The liquidator cannot borrow");
        // }

        // let supported_tokens: Vec<Symbol> = get_supported_tokens(env.clone());

        // if !supported_tokens.contains(denom.clone()) {
        //     panic!("There is no such supported token yet");
        // }

        let available_to_borrow_amount: u128 =
            get_available_to_borrow(env.clone(), user.clone(), denom.clone());

        if amount > available_to_borrow_amount {
            panic!("The amount to be borrowed is not available");
        }

        //     assert!(
        //         get_available_liquidity_by_token(env.clone(), denom.clone())
        //             .unwrap()
        //             .u128()
        //             >= amount.u128()
        //     );

        execute_update_liquidity_index_data(env.clone(), denom.clone());

        let user_borrow_amount_with_interest: u128 =
            get_user_borrow_amount_with_interest(env.clone(), user.clone(), denom.clone());

        let user_borrowing_info: UserBorrowingInfo =
            get_user_borrowing_info(env.clone(), user.clone(), denom.clone());

        let new_user_borrow_amount: u128 = user_borrow_amount_with_interest + amount;

        let current_interest_rate: u128 = get_interest_rate(env.clone(), denom.clone());

        let borrowed_token_decimals: u32 = get_token_decimal(env.clone(), denom.clone());

        let average_interest_rate: u128 = (Decimal::from_i128_with_scale(
            user_borrow_amount_with_interest as i128,
            borrowed_token_decimals,
        )
        .mul(Decimal::from_i128_with_scale(
            user_borrowing_info.average_interest_rate as i128,
            INTEREST_RATE_DECIMALS,
        ))
        .add(
            Decimal::from_i128_with_scale(amount as i128, borrowed_token_decimals).mul(
                Decimal::from_i128_with_scale(
                    current_interest_rate as i128,
                    INTEREST_RATE_DECIMALS,
                ),
            ),
        ))
        .div(Decimal::from_i128_with_scale(
            new_user_borrow_amount as i128,
            borrowed_token_decimals,
        ))
        .to_u128_with_decimals(INTEREST_RATE_DECIMALS)
        .unwrap();

        // updating user borrowing info
        let new_user_borrowing_info: UserBorrowingInfo = UserBorrowingInfo {
            borrowed_amount: new_user_borrow_amount.clone(),
            average_interest_rate: average_interest_rate,
            timestamp: env.ledger().timestamp(),
        };

        let total_borrow_data: TotalBorrowData = get_total_borrow_data(env.clone(), denom.clone());

        let expected_annual_interest_income: u128 = total_borrow_data
            .expected_annual_interest_income
            - Decimal::from_i128_with_scale(
                user_borrowing_info.borrowed_amount as i128,
                borrowed_token_decimals,
            )
            .mul(Decimal::from_i128_with_scale(
                (user_borrowing_info.average_interest_rate / HUNDRED) as i128,
                INTEREST_RATE_DECIMALS,
            ))
            .to_u128_with_decimals(INTEREST_RATE_DECIMALS)
            .unwrap()
            + Decimal::from_i128_with_scale(
                new_user_borrow_amount as i128,
                borrowed_token_decimals,
            )
            .mul(Decimal::from_i128_with_scale(
                (average_interest_rate / HUNDRED) as i128,
                INTEREST_RATE_DECIMALS,
            ))
            .to_u128_with_decimals(INTEREST_RATE_DECIMALS)
            .unwrap();

        let total_borrowed_amount: u128 = total_borrow_data.total_borrowed_amount
            - user_borrowing_info.borrowed_amount
            + new_user_borrow_amount;

        let total_average_interest_rate = HUNDRED
            * Decimal::from_i128_with_scale(
                expected_annual_interest_income as i128,
                INTEREST_RATE_DECIMALS,
            )
            .div(Decimal::from_i128_with_scale(
                total_borrowed_amount as i128,
                borrowed_token_decimals,
            ))
            .to_u128_with_decimals(INTEREST_RATE_DECIMALS)
            .unwrap();

        let new_total_borrow_data: TotalBorrowData = TotalBorrowData {
            denom: denom.clone(),
            total_borrowed_amount: total_borrowed_amount,
            expected_annual_interest_income: expected_annual_interest_income,
            average_interest_rate: total_average_interest_rate,
            timestamp: env.ledger().timestamp(),
        };

        env.storage().persistent().set(
            &DataKey::USER_BORROWING_INFO(user.clone(), denom.clone()),
            &new_user_borrowing_info,
        );
        env.storage().persistent().bump(
            &DataKey::USER_BORROWING_INFO(user.clone(), denom.clone()),
            MONTH_LIFETIME_THRESHOLD,
            MONTH_BUMP_AMOUNT,
        );

        let mut total_borrow_map: Map<Symbol, TotalBorrowData> = env.storage().persistent().get(
            &DataKey::TOTAL_BORROW_DATA).unwrap_or(Map::new(&env));
        total_borrow_map.set(denom.clone(), new_total_borrow_data);
        env.storage().persistent().set(
            &DataKey::TOTAL_BORROW_DATA,
            &total_borrow_map,
        );
        env.storage().persistent().bump(
            &DataKey::TOTAL_BORROW_DATA,
            MONTH_LIFETIME_THRESHOLD,
            MONTH_BUMP_AMOUNT,
        );

        move_token(
            &env,
            &get_token_address(env.clone(), denom.clone()),
            &env.current_contract_address(),
            &user,
            amount as i128,
        )
    }

    pub fn Redeem(env: Env, user: Address, denom: Symbol, mut amount: u128) {
        user.require_auth();

        // let supported_tokens: Vec<Symbol> = get_supported_tokens(env.clone());

        // if !supported_tokens.contains(denom.clone()) {
        //     panic!("There is no such supported token yet");
        // }

        execute_update_liquidity_index_data(env.clone(), denom.clone());

        let current_balance = get_deposit(env.clone(), user.clone(), denom.clone());

        if amount > current_balance {
            panic!("The account doesn't have enough digital tokens to do withdraw");
        }

        if amount == 0 {
            amount = current_balance;
        }

        let remaining: u128 = current_balance - amount;

        let token_decimals: u32 = get_token_decimal(env.clone(), denom.clone());

        let mm_token_price: u128 = get_mm_token_price(env.clone(), denom.clone());

        let new_user_mm_token_balance: u128 =
            Decimal::from_i128_with_scale(remaining as i128, token_decimals)
                .div(Decimal::from_i128_with_scale(
                    mm_token_price as i128,
                    token_decimals,
                ))
                .to_u128_with_decimals(token_decimals)
                .unwrap();

        env.storage().persistent().set(
            &DataKey::USER_MM_TOKEN_BALANCE(user.clone(), denom.clone()),
            &new_user_mm_token_balance,
        );
        env.storage().persistent().bump(
            &DataKey::USER_MM_TOKEN_BALANCE(user.clone(), denom.clone()),
            MONTH_LIFETIME_THRESHOLD,
            MONTH_BUMP_AMOUNT,
        );

        move_token(
            &env,
            &get_token_address(env.clone(), denom.clone()),
            &env.current_contract_address(),
            &user,
            amount as i128,
        )
    }

    pub fn Repay(env: Env, user: Address, repay_token: Symbol, mut repay_amount: u128) {
        user.require_auth();

        let token_address: Address = get_token_address(env.clone(), repay_token.clone());
        move_token(
            &env,
            &token_address,
            &user,
            &env.current_contract_address(),
            repay_amount.clone() as i128,
        );

        // let supported_tokens: Vec<Symbol> = get_supported_tokens(env.clone());

        // if !supported_tokens.contains(repay_token.clone()) {
        //     panic!("There is no such supported token yet");
        // }

        let user_borrowing_info =
            get_user_borrowing_info(env.clone(), user.clone(), repay_token.clone());

        execute_update_liquidity_index_data(env.clone(), repay_token.clone());

        let user_borrow_amount_with_interest =
            get_user_borrow_amount_with_interest(env.clone(), user.clone(), repay_token.clone());

        if repay_amount == 0 {
            repay_amount = user_borrow_amount_with_interest;
        }

        let mut remaining_amount: u128 = 0u128;
        let mut average_interest_rate: u128 = user_borrowing_info.average_interest_rate;
        if repay_amount >= user_borrow_amount_with_interest {
            remaining_amount = repay_amount - user_borrow_amount_with_interest;
            repay_amount = user_borrow_amount_with_interest;
            average_interest_rate = 9_u128;
        }

        let new_user_borrowing_info: UserBorrowingInfo = UserBorrowingInfo {
            borrowed_amount: (user_borrow_amount_with_interest - repay_amount),
            average_interest_rate: average_interest_rate,
            timestamp: env.ledger().timestamp(),
        };

        let total_borrow_data: TotalBorrowData =
            get_total_borrow_data(env.clone(), repay_token.clone());

        let repay_token_decimals: u32 = get_token_decimal(env.clone(), repay_token.clone());

        let expected_annual_interest_income = total_borrow_data.expected_annual_interest_income
            + Decimal::from_i128_with_scale(
                (user_borrow_amount_with_interest - user_borrowing_info.borrowed_amount) as i128,
                repay_token_decimals,
            )
            .mul(Decimal::from_i128_with_scale(
                (user_borrowing_info.average_interest_rate / HUNDRED) as i128,
                INTEREST_RATE_DECIMALS,
            ))
            .to_u128_with_decimals(INTEREST_RATE_DECIMALS)
            .unwrap()
            - Decimal::from_i128_with_scale((repay_amount) as i128, repay_token_decimals)
                .mul(Decimal::from_i128_with_scale(
                    (user_borrowing_info.average_interest_rate / HUNDRED) as i128,
                    INTEREST_RATE_DECIMALS,
                ))
                .to_u128_with_decimals(INTEREST_RATE_DECIMALS)
                .unwrap();

        let total_borrowed_amount: u128 = total_borrow_data.total_borrowed_amount
            + user_borrow_amount_with_interest
            - user_borrowing_info.borrowed_amount
            - repay_amount;

        let mut total_average_interest_rate: u128 = 0u128;
        if total_borrowed_amount != 0u128 {
            total_average_interest_rate = HUNDRED
                * Decimal::from_i128_with_scale(
                    expected_annual_interest_income as i128,
                    INTEREST_RATE_DECIMALS,
                )
                .div(Decimal::from_i128_with_scale(
                    total_borrowed_amount as i128,
                    repay_token_decimals,
                ))
                .to_u128_with_decimals(INTEREST_RATE_DECIMALS)
                .unwrap();
        }

        let new_total_borrow_data = TotalBorrowData {
            denom: repay_token.clone(),
            total_borrowed_amount: total_borrowed_amount,
            expected_annual_interest_income: expected_annual_interest_income,
            average_interest_rate: total_average_interest_rate,
            timestamp: env.ledger().timestamp(),
        };

        env.storage().persistent().set(
            &DataKey::USER_BORROWING_INFO(user.clone(), repay_token.clone()),
            &new_user_borrowing_info,
        );
        env.storage().persistent().bump(
            &DataKey::USER_BORROWING_INFO(user.clone(), repay_token.clone()),
            MONTH_LIFETIME_THRESHOLD,
            MONTH_BUMP_AMOUNT,
        );

        let mut total_borrow_map: Map<Symbol, TotalBorrowData> = env.storage().persistent().get(
            &DataKey::TOTAL_BORROW_DATA).unwrap_or(Map::new(&env));
        total_borrow_map.set(repay_token.clone(), new_total_borrow_data);
        env.storage().persistent().set(
            &DataKey::TOTAL_BORROW_DATA,
            &total_borrow_map,
        );
        env.storage().persistent().bump(
            &DataKey::TOTAL_BORROW_DATA,
            MONTH_LIFETIME_THRESHOLD,
            MONTH_BUMP_AMOUNT,
        );

        if remaining_amount > 0 {
            move_token(
                &env,
                &token_address,
                &env.current_contract_address(),
                &user,
                remaining_amount as i128,
            );
        }
    }

    pub fn Liquidation(env: Env, user: Address) {
        // liquidator only
        let liquidator: Address = read_liquidator(&env);
        liquidator.require_auth();

        let user_utilization_rate = get_user_utilization_rate(env.clone(), user.clone());

        let user_liquidation_threshold: u128 =
            get_user_liquidation_threshold(env.clone(), user.clone());

        assert!(
            user_utilization_rate >= user_liquidation_threshold,
            "User borrowing has not reached the threshold of liquidation"
        );

        for token in get_supported_tokens(env.clone()) {
            execute_update_liquidity_index_data(env.clone(), token.clone());

            let use_user_deposit_as_collateral =
                user_deposit_as_collateral(env.clone(), user.clone(), token.clone());

            let mut user_token_balance = 0u128;
            if use_user_deposit_as_collateral {
                user_token_balance = get_deposit(env.clone(), user.clone(), token.clone());

                env.storage().persistent().set(
                    &DataKey::USER_MM_TOKEN_BALANCE(user.clone(), token.clone()),
                    &0_u128,
                );
                env.storage().persistent().bump(
                    &DataKey::USER_MM_TOKEN_BALANCE(user.clone(), token.clone()),
                    MONTH_LIFETIME_THRESHOLD,
                    MONTH_BUMP_AMOUNT,
                );
            }

            let user_borrow_amount_with_interest =
                get_user_borrow_amount_with_interest(env.clone(), user.clone(), token.clone());

            if user_borrow_amount_with_interest > 0 || user_token_balance > 0 {
                let liquidator_balance =
                    get_deposit(env.clone(), liquidator.clone(), token.clone());

                let token_decimals = get_token_decimal(env.clone(), token.clone());

                if user_borrow_amount_with_interest > 0 {
                    assert!(
                        liquidator_balance >= user_borrow_amount_with_interest,
                        "The liquidator does not have enough deposit balance for liquidation"
                    );

                    let user_borrowing_info =
                        get_user_borrowing_info(env.clone(), user.clone(), token.clone());

                    let new_user_borrowing_info = UserBorrowingInfo {
                        borrowed_amount: 0_u128,
                        average_interest_rate: 0_u128,
                        timestamp: env.ledger().timestamp(),
                    };

                    let total_borrow_data = get_total_borrow_data(env.clone(), token.clone());

                    let expected_annual_interest_income = total_borrow_data
                        .expected_annual_interest_income
                        - Decimal::from_i128_with_scale(
                            (user_borrowing_info.borrowed_amount) as i128,
                            token_decimals,
                        )
                        .mul(Decimal::from_i128_with_scale(
                            (user_borrowing_info.average_interest_rate / HUNDRED) as i128,
                            INTEREST_RATE_DECIMALS,
                        ))
                        .to_u128_with_decimals(INTEREST_RATE_DECIMALS)
                        .unwrap();

                    let total_borrowed_amount = total_borrow_data.total_borrowed_amount
                        - user_borrowing_info.borrowed_amount;

                    let mut total_average_interest_rate = 0u128;
                    if total_borrowed_amount != 0u128 {
                        total_average_interest_rate = HUNDRED
                            * Decimal::from_i128_with_scale(
                                expected_annual_interest_income as i128,
                                INTEREST_RATE_DECIMALS,
                            )
                            .div(Decimal::from_i128_with_scale(
                                total_borrowed_amount as i128,
                                token_decimals,
                            ))
                            .to_u128_with_decimals(INTEREST_RATE_DECIMALS)
                            .unwrap();
                    }

                    let new_total_borrow_data = TotalBorrowData {
                        denom: token.clone(),
                        total_borrowed_amount: total_borrowed_amount,
                        expected_annual_interest_income: expected_annual_interest_income,
                        average_interest_rate: total_average_interest_rate,
                        timestamp: env.ledger().timestamp(),
                    };

                    env.storage().persistent().set(
                        &DataKey::USER_BORROWING_INFO(user.clone(), token.clone()),
                        &new_user_borrowing_info,
                    );
                    env.storage().persistent().bump(
                        &DataKey::USER_BORROWING_INFO(user.clone(), token.clone()),
                        MONTH_LIFETIME_THRESHOLD,
                        MONTH_BUMP_AMOUNT,
                    );
                    let mut total_borrow_map: Map<Symbol, TotalBorrowData> = env.storage().persistent().get( &DataKey::TOTAL_BORROW_DATA).unwrap_or(Map::new(&env));
                    total_borrow_map.set(token.clone(), new_total_borrow_data);
                    env.storage().persistent().set(
                        &DataKey::TOTAL_BORROW_DATA,
                        &total_borrow_map,
                    );
                    env.storage().persistent().bump(
                        &DataKey::TOTAL_BORROW_DATA,
                        MONTH_LIFETIME_THRESHOLD,
                        MONTH_BUMP_AMOUNT,
                    );
                }

                let new_liquidator_token_balance: u128 =
                    liquidator_balance + user_token_balance - user_borrow_amount_with_interest;

                let mm_token_price = get_mm_token_price(env.clone(), token.clone());

                let new_liquidator_mm_token_balance = Decimal::from_i128_with_scale(
                    new_liquidator_token_balance as i128,
                    token_decimals,
                )
                .div(Decimal::from_i128_with_scale(
                    mm_token_price as i128,
                    token_decimals,
                ))
                .to_u128_with_decimals(token_decimals)
                .unwrap();

                env.storage().persistent().set(
                    &DataKey::USER_MM_TOKEN_BALANCE(liquidator.clone(), token.clone()),
                    &new_liquidator_mm_token_balance,
                );
                env.storage().persistent().bump(
                    &DataKey::USER_MM_TOKEN_BALANCE(liquidator.clone(), token.clone()),
                    MONTH_LIFETIME_THRESHOLD,
                    MONTH_BUMP_AMOUNT,
                );
            }
        }
    }

    pub fn GetDeposit(env: Env, user: Address, denom: Symbol) -> u128 {
        get_deposit(env, user, denom)
    }

    pub fn GetTotalBorrowData(env: Env, denom: Symbol) -> TotalBorrowData {
        get_total_borrow_data(env, denom)
    }

    pub fn GetTotalReservesByToken(env: Env, denom: Symbol) -> u128 {
        get_total_reserves_by_token(env, denom)
    }

    pub fn GetUserDepositedUsd(env: Env, user: Address) -> u128 {
        get_user_deposited_usd(env, user)
    }

    pub fn GetMmTokenPrice(env: Env, denom: Symbol) -> u128 {
        get_mm_token_price(env, denom)
    }

    pub fn GetPrice(env: Env, denom: Symbol) -> u128 {
        fetch_price_by_token(env, denom)
    }

    pub fn GetLiquidityRate(env: Env, denom: Symbol) -> u128 {
        get_liquidity_rate(env, denom)
    }

    pub fn GetUserBorrowAmountWithInterest(env: Env, user: Address, denom: Symbol) -> u128 {
        get_user_borrow_amount_with_interest(env, user, denom)
    }

    pub fn GetUserMaxAllowedBorrowAmountUsd(env: Env, user: Address) -> u128 {
        get_user_max_allowed_borrow_amount_usd(env, user)
    }

    pub fn GetUserBorrowedUsd(env: Env, user: Address) -> u128 {
        get_user_borrowed_usd(env, user)
    }

    pub fn GetUserLiquidationThreshold(env: Env, user: Address) -> u128 {
        get_user_liquidation_threshold(env, user)
    }

    pub fn GetUserUtilizationRate(env: Env, user: Address) -> u128 {
        get_user_utilization_rate(env, user)
    }
    pub fn GetAvailableToBorrow(env: Env, user: Address, denom: Symbol) -> u128 {
        get_available_to_borrow(env, user, denom)
    }

    pub fn GetAvailableToRedeem(env: Env, user: Address, denom: Symbol) -> u128 {
        get_available_to_redeem(env, user, denom)
    }

    pub fn GetUserCollateralUsd(env: Env, user: Address) -> u128 {
        get_user_collateral_usd(env, user)
    }

    pub fn GetReserveConfiguration(env: Env, denom: Symbol) -> ReserveConfiguration {
        get_reserve_configuration(env, denom)
    }

    pub fn SetReserveConfiguration(
        env: Env,
        denom: Symbol,
        loan_to_value_ratio: u128,
        liquidation_threshold: u128,
    ) {
        let admin: Address = read_administrator(&env);
        admin.require_auth();

        let supported_tokens: Vec<Symbol> = get_supported_tokens(env.clone());

        if !supported_tokens.contains(denom.clone()) {
            panic!("There is no such supported token yet");
        }

        env.storage().persistent().set(
            &DataKey::RESERVE_CONFIGURATION(denom.clone()),
            &ReserveConfiguration {
                denom: denom.clone(),
                loan_to_value_ratio,
                liquidation_threshold,
            },
        );
        env.storage().persistent().bump(
            &DataKey::RESERVE_CONFIGURATION(denom.clone()),
            MONTH_LIFETIME_THRESHOLD,
            MONTH_BUMP_AMOUNT,
        );
    }

    pub fn SetTokenInterestRateModelParams(
        env: Env,
        denom: Symbol,
        min_interest_rate: u128,
        safe_borrow_max_rate: u128,
        rate_growth_factor: u128,
        optimal_utilization_ratio: u128,
    ) {
        let admin: Address = read_administrator(&env);
        admin.require_auth();

        let supported_tokens: Vec<Symbol> = get_supported_tokens(env.clone());

        if !supported_tokens.contains(denom.clone()) {
            panic!("There is no such supported token yet");
        }

        env.storage().persistent().set(
            &DataKey::TOKENS_INTEREST_RATE_MODEL_PARAM(denom.clone()),
            &TokenInterestRateModelParams {
                denom: denom.clone(),
                min_interest_rate,
                safe_borrow_max_rate,
                rate_growth_factor,
                optimal_utilization_ratio,
            },
        );
        env.storage().persistent().bump(
            &DataKey::TOKENS_INTEREST_RATE_MODEL_PARAM(denom.clone()),
            MONTH_LIFETIME_THRESHOLD,
            MONTH_BUMP_AMOUNT,
        );
    }

    pub fn GetInterestRate(env: Env, denom: Symbol) -> u128 {
        get_interest_rate(env, denom)
    }

    pub fn GetUserBorrowingInfo(env: Env, user: Address, denom: Symbol) -> UserBorrowingInfo {
        get_user_borrowing_info(env, user, denom)
    }

    pub fn UserDepositAsCollateral(env: Env, user: Address, denom: Symbol) -> bool {
        user_deposit_as_collateral(env, user, denom)
    }

    pub fn GetAvailableLiquidityByToken(env: Env, denom: Symbol) -> u128 {
        get_available_liquidity_by_token(env, denom)
    }

    pub fn GetUtilizationRateByToken(env: Env, denom: Symbol) -> u128 {
        get_utilization_rate_by_token(env, denom)
    }

    pub fn GetTotalBorrowedByToken(env: Env, denom: Symbol) -> u128 {
        get_total_borrowed_by_token(env, denom)
    }

    pub fn GetTVL(env: Env) -> u128 {
        let supported_tokens: Vec<Symbol> = get_supported_tokens(env.clone());
        let mut tvl_usd: u128 = 0;
        for token in supported_tokens {
            let token_decimals: u32 = get_token_decimal(env.clone(), token.clone());
            let price: u128 = fetch_price_by_token(env.clone(), token.clone());
            let liquidity: u128 = get_available_liquidity_by_token(env.clone(), token.clone());
            tvl_usd += price * liquidity / 10_u128.pow(token_decimals);
        }
        tvl_usd
    }

    // pub fn GetAllUsersWithBorrows(env: Env) -> Vec<Address> {
    //     get_all_users_with_borrows(env)
    // }
}

mod test;
