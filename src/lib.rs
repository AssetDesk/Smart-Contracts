#![no_std]
#![feature(alloc_error_handler)]

// Testing
// extern crate std;
// use std::println;

use crate::types::{
    DataKey, LiquidityIndexData, ReserveConfiguration, TokenInfo, TokenInterestRateModelParams,
    TotalBorrowData, UserBorrowingInfo, MONTH_LIFETIME_THRESHOLD, MONTH_BUMP_AMOUNT,
};

use soroban_sdk::{
    contract, contracterror, contractimpl, panic_with_error, symbol_short, token, vec, Address,
    Env, Symbol, Vec,
};

#[macro_use]
extern crate alloc;

use crate::alloc::string::ToString;
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

// impl DecimalExt for Decimal {
//     // converting high-precise numbers into u128
//     fn to_u128_with_decimals(&self, decimals: u32) -> Result<u128, rust_decimal::Error> {
//         Ok(self.to_u128().unwrap_or(0))
//     }
// }

fn has_administrator(e: &Env) -> bool {
    let key = DataKey::Admin;
    e.storage().persistent().has(&key)
}

fn read_administrator(e: &Env) -> Address {
    let key = DataKey::Admin;
    e.storage().persistent().get(&key).unwrap()
}

fn write_administrator(e: &Env, id: &Address) {
    let key = DataKey::Admin;
    e.storage().persistent().set(&key, id);
    e.storage()
        .persistent()
        .bump(&key, MONTH_LIFETIME_THRESHOLD, MONTH_BUMP_AMOUNT);
}

fn get_deposit(
    env: Env,
    user: Address,
    denom: Symbol,
) -> u128 {
    // calculates user deposit including deposit interest
    let token_decimals = get_token_decimal(env.clone(), denom.clone());

    let user_mm_token_balance: u128 = env
        .storage()
        .persistent()
        .get(&DataKey::USER_MM_TOKEN_BALANCE(
            user.clone(),
            denom.clone(),
        ))
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
    let token_info: TokenInfo = env
        .storage()
        .persistent()
        .get(&DataKey::SUPPORTED_TOKENS(denom.clone()))
        .unwrap();
    token_balance(&env, &token_info.address, &contract_address) as u128
}

fn get_total_borrow_data(env: Env, denom: Symbol) -> TotalBorrowData {
    let total_borrow_data: TotalBorrowData = env
        .storage()
        .persistent()
        .get(&DataKey::TOTAL_BORROW_DATA(denom.clone()))
        .unwrap();
    total_borrow_data
}

pub fn get_interest_rate(env: Env, denom: Symbol) -> u128 {

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

pub fn get_token_decimal(env: Env, denom: Symbol) -> u32 {
    let token_info: TokenInfo = env
        .storage()
        .persistent()
        .get(&DataKey::SUPPORTED_TOKENS(denom.clone()))
        .unwrap();
    token_info.decimals
}

pub fn get_token_address(env: Env, denom: Symbol) -> Address {
    let token_info: TokenInfo = env
        .storage()
        .persistent()
        .get(&DataKey::SUPPORTED_TOKENS(denom.clone()))
        .unwrap();
    token_info.address
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

pub fn get_utilization_rate_by_token( env: Env, denom: Symbol) -> u128 {
    let reserves_by_token = get_total_reserves_by_token(env.clone(), denom.clone());

    if reserves_by_token != 0 {
        let borrowed_by_token = get_total_borrowed_by_token(env, denom.clone());

            borrowed_by_token * HUNDRED_PERCENT / reserves_by_token
    } else {
        0_u128
    }
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

fn get_user_borrowing_info(
    env: Env,
    user: Address,
    denom: Symbol,
) -> UserBorrowingInfo {
    let user_borrowing_info: UserBorrowingInfo = env.storage()
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

pub fn get_user_borrow_amount_with_interest(
    env: Env,
    user: Address,
    denom: Symbol,
) -> u128 {
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

fn user_deposit_as_collateral(env: Env, user: Address, denom: Symbol) -> bool {
    let use_user_deposit_as_collateral: bool =  env.storage().persistent().get(
        &DataKey::USER_DEPOSIT_AS_COLLATERAL(user.clone(), denom.clone())
    ).unwrap_or(false);

    use_user_deposit_as_collateral
}

pub fn fetch_price_by_token(env: Env, denom: Symbol) -> u128 {
    env.storage().persistent().get(&DataKey::PRICES(denom.clone())).unwrap_or(0_u128)
    }

pub fn get_user_collateral_usd(env: Env, user: Address) -> u128 {
    let mut user_collateral_usd = 0_u128;

    for token in get_supported_tokens(env.clone()) {
        let use_user_deposit_as_collateral =
            user_deposit_as_collateral(env.clone(), user.clone(), token.clone());

        if use_user_deposit_as_collateral {
            let user_deposit = get_deposit(env.clone(),  user.clone(), token.clone());

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

pub fn get_user_borrowed_usd(env: Env, user: Address) -> u128 {
    let mut user_borrowed_usd: u128 = 0_u128;
    for token in get_supported_tokens(env.clone()) {
        let user_borrow_amount_with_interest = get_user_borrow_amount_with_interest(
            env.clone(),
            user.clone(),
            token.clone(),
        );

        let token_decimals = get_token_decimal(env.clone(), token.clone());

        let price = fetch_price_by_token(env.clone(), token.clone());

        user_borrowed_usd += Decimal::from_i128_with_scale(
            user_borrow_amount_with_interest as i128,
            token_decimals,
        )
            .mul(Decimal::from_i128_with_scale(price as i128, USD_DECIMALS))
            .to_u128_with_decimals(USD_DECIMALS)
            .unwrap()
    }

    user_borrowed_usd
}

pub fn get_user_max_allowed_borrow_amount_usd(
    env: Env,
    user: Address,
) -> u128 {
    // the maximum amount in USD that a user can borrow
    let mut max_allowed_borrow_amount_usd = 0u128;

    for token in get_supported_tokens(env.clone()) {
        let use_user_deposit_as_collateral =
            user_deposit_as_collateral(env.clone(), user.clone(), token.clone());

        if use_user_deposit_as_collateral {
            let user_deposit =
                get_deposit(env.clone(), user.clone(), token.clone());

            let reserve_configuration: ReserveConfiguration = env.storage().persistent().get(
                &DataKey::RESERVE_CONFIGURATION(token.clone())
            ).unwrap();

            let loan_to_value_ratio = reserve_configuration.loan_to_value_ratio;

            let token_decimals = get_token_decimal(env.clone(), token.clone());

            let price = fetch_price_by_token(env.clone(), token.clone());

            let user_deposit_usd =
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

pub fn get_available_to_borrow(
    env: Env,
    user: Address,
    denom: Symbol,
) -> u128 {
    let mut available_to_borrow = 0u128;

    // maximum amount allowed for borrowing
    let max_allowed_borrow_amount_usd =
        get_user_max_allowed_borrow_amount_usd(env.clone(), user.clone());

    let sum_user_borrow_balance_usd = get_user_borrowed_usd( env.clone(), user.clone());

    if max_allowed_borrow_amount_usd > sum_user_borrow_balance_usd {
        let token_decimals = get_token_decimal( env.clone(), denom.clone());

        let price = fetch_price_by_token(env.clone(), denom.clone());

        available_to_borrow = Decimal::from_i128_with_scale(
            (max_allowed_borrow_amount_usd - sum_user_borrow_balance_usd) as i128,
            USD_DECIMALS,
        )
            .div(Decimal::from_i128_with_scale(price as i128, USD_DECIMALS))
            .to_u128_with_decimals(token_decimals)
            .unwrap();

        let token_liquidity =
            get_available_liquidity_by_token(env.clone(), denom.clone());

        if available_to_borrow > token_liquidity {
            available_to_borrow = token_liquidity
        }
    }

    available_to_borrow
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

    pub fn initialize(e: Env, admin: Address) {
        if has_administrator(&e) {
            panic!("already initialized")
        }
        write_administrator(&e, &admin);
    }

    pub fn deposit(env: Env, user_address: Address, denom: Symbol, deposited_token_amount: u128) {

        
        user_address.require_auth();

        let token_address: Address = get_token_address(env.clone(), denom.clone());
        move_token(&env, &token_address, &user_address, &env.current_contract_address(), deposited_token_amount.clone() as i128);

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

        let mut supported_tokens: Vec<Symbol> = get_supported_tokens(env.clone());
        // TODO
        // assert!(
        //     !SUPPORTED_TOKENS.has(deps.storage, denom.clone()),
        //     "There already exists such a supported token"
        // );
        supported_tokens.push_back(denom.clone());
        env.storage()
            .persistent()
            .set(&DataKey::SUPPORTED_TOKENS_LIST, &supported_tokens);

        let token_info: TokenInfo = TokenInfo {
            denom: denom.clone(),
            address,
            name,
            symbol: denom.clone(),
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

    pub fn UpdatePrice(env: Env, denom: Symbol, price: u128) {
        // TODO
        // Admin only
        env.storage().persistent().set(
            &DataKey::PRICES(denom.clone()),
            &price,
        );
    }

    pub fn ToggleCollateralSetting(env: Env, user: Address, denom: Symbol ) {

        user.require_auth();

        let use_user_deposit_as_collateral =
            user_deposit_as_collateral(env.clone(), user.clone(), denom.clone());

        if use_user_deposit_as_collateral {
            let user_token_balance: u128 = get_deposit(env.clone(), user.clone(), denom.clone());

            if user_token_balance != 0 {
                let token_decimals = get_token_decimal(env.clone(), denom.clone());

                let price = fetch_price_by_token( env.clone(), denom.clone());

                let user_token_balance_usd =
                    Decimal::from_i128_with_scale(user_token_balance as i128, token_decimals)
                        .mul(Decimal::from_i128_with_scale(price as i128, USD_DECIMALS))
                        .to_u128_with_decimals(USD_DECIMALS)
                        .unwrap();

                let sum_collateral_balance_usd = get_user_collateral_usd(env.clone(), user.clone());

                let sum_borrow_balance_usd = get_user_borrowed_usd(env.clone(), user.clone());

        //         let user_liquidation_threshold = get_user_liquidation_threshold(
        //             deps.as_ref(),
        //             env.clone(),
        //             info.sender.to_string(),
        //         )
        //             .unwrap()
        //             .u128();

        //         assert!(
        //             sum_borrow_balance_usd * HUNDRED_PERCENT / user_liquidation_threshold < sum_collateral_balance_usd - user_token_balance_usd,
        //             "The collateral has already using to collateralise the borrowing. Not enough available balance"
        //         );
            }
        }

        env.storage().persistent().set(
            &DataKey::USER_DEPOSIT_AS_COLLATERAL(user.clone(), denom.clone()),
            &!use_user_deposit_as_collateral,
        );

    }

    pub fn Borrow(env: Env, user: Address, denom: Symbol, amount: u128) {
        user.require_auth();

        // TODO
    //     assert_ne!(
    //         info.sender.to_string(),
    //         LIQUIDATOR.load(deps.storage).unwrap(),
    //         "The liquidator cannot borrow"
    //     );

    //     assert!(
    //         SUPPORTED_TOKENS.has(deps.storage, denom.clone()),
    //         "There is no such supported token yet"
    //     );


        let available_to_borrow_amount: u128 = get_available_to_borrow(
            env.clone(),
            user.clone(),
            denom.clone(),
        );

    //     assert!(
    //         available_to_borrow_amount >= amount.u128(),
    //         "The amount to be borrowed is not available"
    //     );

    //     assert!(
    //         get_available_liquidity_by_token(deps.as_ref(), env.clone(), denom.clone())
    //             .unwrap()
    //             .u128()
    //             >= amount.u128()
    //     );

        execute_update_liquidity_index_data(env.clone(), denom.clone());

        let user_borrow_amount_with_interest: u128 = get_user_borrow_amount_with_interest(
            env.clone(),
            user.clone(),
            denom.clone(),
        );

        let user_borrowing_info: UserBorrowingInfo = get_user_borrowing_info(
            env.clone(), 
            user.clone(),
            denom.clone(),
        );

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

        let expected_annual_interest_income: u128 = total_borrow_data.expected_annual_interest_income
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

        env.storage()
            .persistent()
            .set(&DataKey::USER_BORROWING_INFO(user.clone(), denom.clone()), &new_user_borrowing_info);

        env.storage().persistent().set(
            &DataKey::TOTAL_BORROW_DATA(denom.clone()),
            &new_total_borrow_data,
        );

        move_token(&env, &get_token_address(env.clone(), denom.clone()), &env.current_contract_address(), &user, amount as i128)

    }

    pub fn GetDeposit(env: Env, user: Address, denom: Symbol) -> u128 {
        get_deposit(env.clone(), user, denom)
    }

    pub fn GetTotalBorrowData(env: Env, denom: Symbol) -> TotalBorrowData {
        get_total_borrow_data(env.clone(), denom)
    }

    pub fn GetTotalReservesByToken (env: Env, denom: Symbol) -> u128 {
        get_total_reserves_by_token(env.clone(), denom)
    }
}

mod test;