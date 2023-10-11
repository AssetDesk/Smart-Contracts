#![no_std]
#![feature(alloc_error_handler)]

use crate::types::{
    DataKey, LiquidityIndexData, ReserveConfiguration, TokenInfo, TokenInterestRateModelParams,
    TotalBorrowData,
};

use soroban_sdk::{
    contract, contracterror, contractimpl, panic_with_error, symbol_short, token, vec, Address,
    Env, Symbol, Vec,
};

use crate::alloc::string::ToString;
use core::ops::{Add, Div, Mul};
use rust_decimal::prelude::{Decimal, MathematicalOps};

use linked_list_allocator::LockedHeap;

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

#[macro_use]
extern crate alloc;

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

impl DecimalExt for Decimal {
    // converting high-precise numbers into u128
    fn to_u128_with_decimals(&self, decimals: u32) -> Result<u128, rust_decimal::Error> {
        let s = self.to_string();
        let (left, right) = s.split_once(".").unwrap_or((&s, ""));
        let mut right = right.to_string();
        let right_len = right.len() as u32;
        if right_len > decimals {
            right.truncate(decimals.try_into().unwrap());
        } else if right_len < decimals {
            let zeroes = decimals - right_len;
            right.push_str(&"0".repeat(zeroes.try_into().unwrap()));
        }
        let s = format!("{}{}", left, right);
        Ok(s.parse::<u128>().unwrap_or(0))
    }
}

fn get_available_liquidity_by_token(env: Env, denom: Symbol) -> u128 {
    let contract_address = env.current_contract_address();

    10_000_000_u128
}

fn get_total_borrow_data(env: Env, denom: Symbol) -> TotalBorrowData {
    let total_borrow_data: TotalBorrowData = env
        .storage()
        .persistent()
        .get(&DataKey::TOTAL_BORROW_DATA(denom.clone()))
        .unwrap();
    total_borrow_data
}

pub fn get_token_decimal(env: Env, denom: Symbol) -> u32 {
    let token_info: TokenInfo = env
        .storage()
        .persistent()
        .get(&DataKey::SUPPORTED_TOKENS(denom.clone()))
        .unwrap();
    token_info.decimals
}

fn get_total_borrowed_by_token(env: Env, denom: Symbol) -> u128 {
    let total_borrow_data = get_total_borrow_data(env.clone(), denom.clone());

    let token_decimals: u32 = get_token_decimal(env.clone(), denom.clone());

    let total_borrowed_amount_with_interest: u128 = calc_borrow_amount_with_interest(
        total_borrow_data.total_borrowed_amount,
        total_borrow_data.average_interest_rate,
        (env.ledger().timestamp() - total_borrow_data.timestamp) as u128,
        token_decimals,
    );

    total_borrowed_amount_with_interest
}

pub fn calc_borrow_amount_with_interest(
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

pub fn get_current_liquidity_index_ln(env: Env, denom: Symbol) -> u128 {
    let liquidity_rate: u128 = get_liquidity_rate(env.clone(), denom.clone());
    let liquidity_index_data: LiquidityIndexData = env
        .storage()
        .persistent()
        .get(&DataKey::LIQUIDITY_INDEX_DATA(denom.clone()))
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

    env.storage().persistent().set(
        &DataKey::LIQUIDITY_INDEX_DATA(denom.clone()),
        &new_liquidity_index_data,
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

    // u128::try_from(mm_token_price).unwrap()
}

#[contract]
pub struct LendingContract;

#[contractimpl]
impl LendingContract {
    pub fn deposit(env: Env, user_address: Address) {
        let denom = Symbol::new(&env, "USDT");
        let deposited_token_amount = 100_u128;

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
    }

    pub fn AddMarkets(
        env: Env,
        denom: Symbol,
        name: Symbol,
        symbol: Symbol,
        decimals: u32,
        loan_to_value_ratio: u128,
        liquidation_threshold: u128,
        min_interest_rate: u128,
        safe_borrow_max_rate: u128,
        rate_growth_factor: u128,
        optimal_utilization_ratio: u128,
    ) {
        // assert!(
        //     !SUPPORTED_TOKENS.has(deps.storage, denom.clone()),
        //     "There already exists such a supported token"
        // );

        let token_info: TokenInfo = TokenInfo {
            denom: denom.clone(),
            name,
            symbol,
            decimals,
        };
        env.storage()
            .persistent()
            .set(&DataKey::SUPPORTED_TOKENS(denom.clone()), &token_info);

        let reserve_configuration: ReserveConfiguration = ReserveConfiguration {
            denom: denom.clone(),
            loan_to_value_ratio,
            liquidation_threshold,
        };
        env.storage().persistent().set(
            &DataKey::RESERVE_CONFIGURATION(denom.clone()),
            &reserve_configuration,
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

        let total_borrow_data: TotalBorrowData = TotalBorrowData {
            denom: denom.clone(),
            total_borrowed_amount: 0_u128,
            expected_annual_interest_income: 0_u128,
            average_interest_rate: 0_u128,
            timestamp: env.ledger().timestamp(),
        };
        env.storage().persistent().set(
            &DataKey::TOTAL_BORROW_DATA(denom.clone()),
            &total_borrow_data,
        );

        let liquidity_index_data: LiquidityIndexData = LiquidityIndexData {
            denom: denom.clone(),
            liquidity_index_ln: 0_u128,
            timestamp: env.ledger().timestamp(),
        };
        env.storage().persistent().set(
            &DataKey::LIQUIDITY_INDEX_DATA(denom.clone()),
            &liquidity_index_data,
        );
    }
}
