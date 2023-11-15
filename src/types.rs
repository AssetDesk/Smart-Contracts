#![allow(non_camel_case_types)]
#![allow(dead_code)]

use soroban_sdk::{contracttype, Address, Symbol};

pub(crate) const DAY_IN_LEDGERS: u32 = 17280;
pub(crate) const WEEK_BUMP_AMOUNT: u32 = 7 * DAY_IN_LEDGERS;
pub(crate) const WEEK_LIFETIME_THRESHOLD: u32 = WEEK_BUMP_AMOUNT - DAY_IN_LEDGERS;

pub(crate) const MONTH_BUMP_AMOUNT: u32 = 30 * DAY_IN_LEDGERS;
pub(crate) const MONTH_LIFETIME_THRESHOLD: u32 = MONTH_BUMP_AMOUNT - DAY_IN_LEDGERS;

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,                            // Address of the Contract admin account
    Liquidator,                       // Address of the liquidator account
    TotalBorrowData,                  // Map of TotalBorrowData per denom
    SupportedTokensInfo,              // Map of TokenInfo denom data
    SupportedTokensList,              // List of supported tokens
    LiquidityIndexData,               // Map of LiquidityIndexData per denom
    UserMMTokenBalance(Address),      // user mm token balance per denom
    ReserveConfiguration,             //Map ReserveConfiguration per denom
    TokensInterestRateModelParams,    // Map TokenInterestRateModelParams per denom
    Prices,                           // Map price for denom
    UserDepositAsCollateral(Address), // Map of bool per denom
    UserBorrowingInfo(Address),       // Map UserBorrowingInfo per denom
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
