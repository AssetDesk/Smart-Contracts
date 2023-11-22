use soroban_sdk::{contracttype, Address, Symbol};

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    // LendingContract,
    VaultContract,
    Admin,
    Liquidator,
    SupportedTokensInfo,
    SupportedTokensList,
    Prices,
    UserBalance(Address),
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
