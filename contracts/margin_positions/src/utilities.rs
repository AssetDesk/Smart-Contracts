use soroban_sdk::{token, Address, Env, Map, Symbol, Vec};

pub(crate) const DAY_IN_LEDGERS: u32 = 17280;
pub(crate) const WEEK_BUMP_AMOUNT: u32 = 7 * DAY_IN_LEDGERS;
pub(crate) const WEEK_LIFETIME_THRESHOLD: u32 = WEEK_BUMP_AMOUNT - DAY_IN_LEDGERS;

pub(crate) const MONTH_BUMP_AMOUNT: u32 = 30 * DAY_IN_LEDGERS;
pub(crate) const MONTH_LIFETIME_THRESHOLD: u32 = MONTH_BUMP_AMOUNT - DAY_IN_LEDGERS;

use crate::storage::*;

pub fn has_admin(e: &Env) -> bool {
    let key = DataKey::Admin;
    e.storage().persistent().has(&key)
}

pub fn get_admin(e: &Env) -> Address {
    let key = DataKey::Admin;
    e.storage().persistent().get(&key).unwrap()
}

pub fn set_admin(e: &Env, admin: &Address) {
    let key = DataKey::Admin;
    e.storage().persistent().set(&key, admin);
    e.storage()
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

pub fn get_token_address(env: Env, denom: Symbol) -> Address {
    let token_info: Map<Symbol, TokenInfo> = env
        .storage()
        .persistent()
        .get(&DataKey::SupportedTokensInfo)
        .unwrap();
    token_info.get(denom).unwrap().address
}

pub fn get_deposit(env: Env, user: Address, denom: Symbol) -> u128 {
    let user_token_balance: u128 = env
        .storage()
        .persistent()
        .get(&DataKey::UserBalance(user.clone()))
        .unwrap_or(Map::new(&env))
        .get(denom.clone())
        .unwrap_or(0_u128);

    user_token_balance
}

pub fn fetch_price_by_token(env: Env, denom: Symbol) -> u128 {
    env.storage()
        .persistent()
        .get(&DataKey::Prices)
        .unwrap_or(Map::new(&env))
        .get(denom.clone())
        .unwrap_or(0_u128)
}

pub fn get_available_to_redeem(env: Env, user: Address, denom: Symbol) -> u128 {
    let mut available_to_redeem: u128 = 0u128;

    let user_token_balance: u128 = get_deposit(env.clone(), user.clone(), denom.clone());
    available_to_redeem = user_token_balance;

    available_to_redeem
}

pub fn get_supported_tokens(env: Env) -> Vec<Symbol> {
    env.storage()
        .persistent()
        .get(&DataKey::SupportedTokensList)
        .unwrap_or(Vec::<Symbol>::new(&env))
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

// pub fn get_lending_contract(e: &Env) -> Address {
//     let key = DataKey::LendingContract;
//     e.storage().persistent().get(&key).unwrap()
// }
//
// pub fn set_lending_contract(e: &Env, lending_contract: &Address) {
//     let key = DataKey::LendingContract;
//     e.storage().persistent().set(&key, lending_contract);
//     e.storage()
//         .persistent()
//         .bump(&key, MONTH_LIFETIME_THRESHOLD, MONTH_BUMP_AMOUNT);
// }
