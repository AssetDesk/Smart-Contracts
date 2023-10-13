use soroban_sdk::{contracttype, Address, Symbol};

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    TOTAL_BORROW_DATA(Symbol),                // TotalBorrowData per denom
    SUPPORTED_TOKENS(Symbol),                 // TokenInfo denom data
    LIQUIDITY_INDEX_DATA(Symbol),             // LiquidityIndexData per denom
    USER_MM_TOKEN_BALANCE(Address, Symbol),   // user mm token balance per denom
    RESERVE_CONFIGURATION(Symbol),            // ReserveConfiguration per denom
    TOKENS_INTEREST_RATE_MODEL_PARAM(Symbol), // TokenInterestRateModelParams per denom
    PRICES(Symbol),                           // price for denom
    USER_DEPOSIT_AS_COLLATERAL(Address, Symbol), // bool
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
