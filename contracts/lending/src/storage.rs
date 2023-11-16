#![allow(dead_code)]

use core::ops::{Div, Mul};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::{Decimal, MathematicalOps};

use soroban_sdk::{contracttype, map, symbol_short, token, Address, Env, Map, Symbol, Vec};

pub(crate) const DAY_IN_LEDGERS: u32 = 17280;
pub(crate) const WEEK_BUMP_AMOUNT: u32 = 7 * DAY_IN_LEDGERS;
pub(crate) const WEEK_LIFETIME_THRESHOLD: u32 = WEEK_BUMP_AMOUNT - DAY_IN_LEDGERS;

pub(crate) const MONTH_BUMP_AMOUNT: u32 = 30 * DAY_IN_LEDGERS;
pub(crate) const MONTH_LIFETIME_THRESHOLD: u32 = MONTH_BUMP_AMOUNT - DAY_IN_LEDGERS;

pub(crate) const PERCENT_DECIMALS: u32 = 5;
pub(crate) const HUNDRED_PERCENT: u128 = 100 * 10u128.pow(PERCENT_DECIMALS);

pub(crate) const INTEREST_RATE_DECIMALS: u32 = 18;
pub(crate) const INTEREST_RATE_MULTIPLIER: u128 = 10u128.pow(INTEREST_RATE_DECIMALS);
pub(crate) const HUNDRED: u128 = 100;
pub(crate) const YEAR_IN_SECONDS: u128 = 31536000; // 365 days

pub(crate) const USD_DECIMALS: u32 = 8;

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

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    VaultContract,
    Admin,
    // Address of the Contract admin account
    Liquidator,
    // Address of the liquidator account
    TotalBorrowData,
    // Map of TotalBorrowData per denom
    SupportedTokensInfo,
    // Map of TokenInfo denom data
    SupportedTokensList,
    // List of supported tokens
    LiquidityIndexData,
    // Map of LiquidityIndexData per denom
    UserMMTokenBalance(Address),
    // user mm token balance per denom
    ReserveConfiguration,
    //Map ReserveConfiguration per denom
    TokensInterestRateModelParams,
    // Map TokenInterestRateModelParams per denom
    Prices,
    // Map price for denom
    UserDepositAsCollateral(Address),
    // Map of bool per denom
    UserBorrowingInfo(Address), // Map UserBorrowingInfo per denom
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct TokenInfo {
    pub denom: Symbol,
    pub address: Address,
    pub name: Symbol,
    pub symbol: Symbol,
    pub decimals: u32,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct LiquidityIndexData {
    pub denom: Symbol,
    pub liquidity_index_ln: u128,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct TotalBorrowData {
    pub denom: Symbol,
    pub total_borrowed_amount: u128,
    pub expected_annual_interest_income: u128,
    pub average_interest_rate: u128,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct UserBorrowingInfo {
    pub borrowed_amount: u128,
    pub average_interest_rate: u128,
    pub timestamp: u64,
}

impl Default for UserBorrowingInfo {
    fn default() -> Self {
        UserBorrowingInfo {
            borrowed_amount: 0_u128,
            average_interest_rate: 0_u128,
            timestamp: 0_u64,
        }
    }
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct ReserveConfiguration {
    pub denom: Symbol,
    pub loan_to_value_ratio: u128,
    // LTV ratio
    pub liquidation_threshold: u128,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct TokenInterestRateModelParams {
    pub denom: Symbol,
    pub min_interest_rate: u128,
    pub safe_borrow_max_rate: u128,
    pub rate_growth_factor: u128,
    pub optimal_utilization_ratio: u128,
}

pub fn has_admin(env: &Env) -> bool {
    let key = DataKey::Admin;
    env.storage().persistent().has(&key)
}

pub fn get_admin(env: &Env) -> Address {
    let key = DataKey::Admin;
    env.storage().persistent().get(&key).unwrap()
}

pub fn set_admin(env: &Env, admin: &Address) {
    let key = DataKey::Admin;
    env.storage().persistent().set(&key, admin);
    env.storage()
        .persistent()
        .bump(&key, MONTH_LIFETIME_THRESHOLD, MONTH_BUMP_AMOUNT);
}

pub fn set_vault_contract(env: &Env, vault_contract: &Address) {
    let key = DataKey::VaultContract;
    env.storage().persistent().set(&key, vault_contract);
    env.storage()
        .persistent()
        .bump(&key, MONTH_LIFETIME_THRESHOLD, MONTH_BUMP_AMOUNT);
}

pub fn get_vault_contract(env: &Env) -> Address {
    let key = DataKey::VaultContract;
    env.storage().persistent().get(&key).unwrap()
}


pub fn get_liquidator(env: &Env) -> Address {
    let key = DataKey::Liquidator;
    env.storage().persistent().get(&key).unwrap()
}

pub fn set_liquidator(env: &Env, liquidator: &Address) {
    let key = DataKey::Liquidator;
    env.storage().persistent().set(&key, liquidator);
    env.storage()
        .persistent()
        .bump(&key, MONTH_LIFETIME_THRESHOLD, MONTH_BUMP_AMOUNT);
}

pub fn get_deposit(env: Env, user: Address, denom: Symbol) -> u128 {
    // calculates user deposit including deposit interest
    let token_decimals = get_token_decimal(env.clone(), denom.clone());

    let user_mm_token_balance: u128 = env
        .storage()
        .persistent()
        .get(&DataKey::UserMMTokenBalance(user.clone()))
        .unwrap_or(Map::new(&env))
        .get(denom.clone())
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

pub fn get_available_liquidity_by_token(env: Env, denom: Symbol) -> u128 {
    let contract_address = get_vault_contract(&env);
    let token_info: Map<Symbol, TokenInfo> = env
        .storage()
        .persistent()
        .get(&DataKey::SupportedTokensInfo)
        .unwrap_or(Map::new(&env));
    token_balance(
        &env,
        &token_info.get(denom).unwrap().address,
        &contract_address,
    ) as u128
}

pub fn get_total_borrow_data(env: Env, denom: Symbol) -> TotalBorrowData {
    let total_borrow_data: Map<Symbol, TotalBorrowData> = env
        .storage()
        .persistent()
        .get(&DataKey::TotalBorrowData)
        .unwrap_or(Map::new(&env));
    total_borrow_data.get(denom).unwrap()
}

pub fn get_interest_rate(env: Env, denom: Symbol) -> u128 {
    let utilization_rate = get_utilization_rate_by_token(env.clone(), denom.clone());

    let token_interest: TokenInterestRateModelParams = env
        .storage()
        .persistent()
        .get::<DataKey, Map<Symbol, TokenInterestRateModelParams>>(
            &DataKey::TokensInterestRateModelParams,
        )
        .unwrap()
        .get(denom.clone())
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
    let token_info: Map<Symbol, TokenInfo> = env
        .storage()
        .persistent()
        .get(&DataKey::SupportedTokensInfo)
        .unwrap();
    token_info.get(denom).unwrap().decimals
}

pub fn get_token_address(env: Env, denom: Symbol) -> Address {
    let token_info: Map<Symbol, TokenInfo> = env
        .storage()
        .persistent()
        .get(&DataKey::SupportedTokensInfo)
        .unwrap();
    token_info.get(denom).unwrap().address
}

pub fn get_supported_tokens(env: Env) -> Vec<Symbol> {
    env.storage()
        .persistent()
        .get(&DataKey::SupportedTokensList)
        .unwrap_or(Vec::<Symbol>::new(&env))
}

pub fn get_total_borrowed_by_token(env: Env, denom: Symbol) -> u128 {
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

pub fn get_user_max_allowed_borrow_amount_usd(env: Env, user: Address) -> u128 {
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
                .get::<DataKey, Map<Symbol, ReserveConfiguration>>(&DataKey::ReserveConfiguration)
                .unwrap()
                .get(token.clone())
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

pub fn get_utilization_rate_by_token(env: Env, denom: Symbol) -> u128 {
    let reserves_by_token = get_total_reserves_by_token(env.clone(), denom.clone());

    if reserves_by_token != 0 {
        let borrowed_by_token = get_total_borrowed_by_token(env, denom.clone());

        borrowed_by_token * HUNDRED_PERCENT / reserves_by_token
    } else {
        0_u128
    }
}

pub fn get_reserve_configuration(env: Env, denom: Symbol) -> ReserveConfiguration {
    let reserve_configuration: ReserveConfiguration = env
        .storage()
        .persistent()
        .get(&DataKey::ReserveConfiguration)
        .unwrap_or(Map::new(&env))
        .get(denom.clone())
        .unwrap();
    reserve_configuration
}

pub fn get_user_utilization_rate(env: Env, user: Address) -> u128 {
    let sum_collateral_balance_usd: u128 = get_user_collateral_usd(env.clone(), user.clone());

    if sum_collateral_balance_usd != 0 {
        let sum_user_borrow_balance_usd: u128 = get_user_borrowed_usd(env.clone(), user.clone());

        sum_user_borrow_balance_usd * HUNDRED_PERCENT / sum_collateral_balance_usd
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

pub fn get_user_borrowing_info(env: Env, user: Address, denom: Symbol) -> UserBorrowingInfo {
    let user_borrowing_info: UserBorrowingInfo = env
        .storage()
        .persistent()
        .get(&DataKey::UserBorrowingInfo(user.clone()))
        .unwrap_or(Map::new(&env))
        .get(denom.clone())
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

pub fn get_user_borrow_amount_with_interest(env: Env, user: Address, denom: Symbol) -> u128 {
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

pub fn get_total_reserves_by_token(env: Env, denom: Symbol) -> u128 {
    let token_liquidity: u128 = get_available_liquidity_by_token(env.clone(), denom.clone());
    let borrowed_by_token: u128 = get_total_borrowed_by_token(env.clone(), denom.clone());
    token_liquidity + borrowed_by_token
}

pub fn get_liquidity_rate(env: Env, denom: Symbol) -> u128 {
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
        .get(&DataKey::LiquidityIndexData)
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

pub fn execute_update_liquidity_index_data(env: Env, denom: Symbol) {
    let current_liquidity_index_ln = get_current_liquidity_index_ln(env.clone(), denom.clone());

    let new_liquidity_index_data = LiquidityIndexData {
        denom: denom.clone(),
        liquidity_index_ln: current_liquidity_index_ln,
        timestamp: env.ledger().timestamp(),
    };

    let mut liquidity_map: Map<Symbol, LiquidityIndexData> = env
        .storage()
        .persistent()
        .get(&DataKey::LiquidityIndexData)
        .unwrap_or(Map::new(&env));
    liquidity_map.set(denom.clone(), new_liquidity_index_data);
    env.storage()
        .persistent()
        .set(&DataKey::LiquidityIndexData, &liquidity_map);
    env.storage().persistent().bump(
        &DataKey::LiquidityIndexData,
        MONTH_LIFETIME_THRESHOLD,
        MONTH_BUMP_AMOUNT,
    );
}

pub fn get_mm_token_price(env: Env, denom: Symbol) -> u128 {
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

pub fn user_deposit_as_collateral(env: Env, user: Address, denom: Symbol) -> bool {
    let use_user_deposit_as_collateral: bool = env
        .storage()
        .persistent()
        .get(&DataKey::UserDepositAsCollateral(user.clone()))
        .unwrap_or(Map::new(&env))
        .get(denom.clone())
        .unwrap_or(false);

    // // POC: Only xlm is used as a collateral
    // let mut use_user_deposit_as_collateral: bool = false;
    // if denom == symbol_short!("xlm") {
    //     use_user_deposit_as_collateral = true;
    // }

    use_user_deposit_as_collateral
}

pub fn fetch_price_by_token(env: Env, denom: Symbol) -> u128 {
    env.storage()
        .persistent()
        .get(&DataKey::Prices)
        .unwrap_or(Map::new(&env))
        .get(denom.clone())
        .unwrap_or(0_u128)
}

pub fn get_user_deposited_usd(env: Env, user: Address) -> u128 {
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

pub fn get_user_collateral_usd(env: Env, user: Address) -> u128 {
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

pub fn get_user_borrowed_usd(env: Env, user: Address) -> u128 {
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

pub fn get_available_to_borrow(env: Env, user: Address, denom: Symbol) -> u128 {
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

pub fn get_available_to_redeem(env: Env, user: Address, denom: Symbol) -> u128 {
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

pub fn get_user_liquidation_threshold(env: Env, user: Address) -> u128 {
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
                .get::<DataKey, Map<Symbol, ReserveConfiguration>>(&DataKey::ReserveConfiguration)
                .unwrap()
                .get(token.clone())
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

pub fn move_token(env: &Env, token: &Address, from: &Address, to: &Address, transfer_amount: i128) {
    // new token interface
    let token_client = token::Client::new(&env, &token);
    token_client.transfer(&from, to, &transfer_amount);
}

pub fn token_balance(env: &Env, token: &Address, user_address: &Address) -> i128 {
    let token_client = token::Client::new(&env, &token);
    token_client.balance(&user_address)
}
