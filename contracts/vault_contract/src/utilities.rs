use soroban_sdk::{token, Address, Env};

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

pub fn get_lending_contract(e: &Env) -> Address {
    let key = DataKey::LendingContract;
    e.storage().persistent().get(&key).unwrap()
}

pub fn set_lending_contract(e: &Env, lending_contract: &Address) {
    let key = DataKey::LendingContract;
    e.storage().persistent().set(&key, lending_contract);
    e.storage()
        .persistent()
        .bump(&key, MONTH_LIFETIME_THRESHOLD, MONTH_BUMP_AMOUNT);
}

pub fn get_margin_contract(e: &Env) -> Address {
    let key = DataKey::MarginPositionsContract;
    e.storage().persistent().get(&key).unwrap()
}

pub fn set_margin_contract(e: &Env, margin_contract: &Address) {
    let key = DataKey::MarginPositionsContract;
    e.storage().persistent().set(&key, margin_contract);
    e.storage()
        .persistent()
        .bump(&key, MONTH_LIFETIME_THRESHOLD, MONTH_BUMP_AMOUNT);
}

pub fn move_token(env: &Env, token: &Address, from: &Address, to: &Address, transfer_amount: i128) {
    // new token interface
    let token_client = token::Client::new(&env, &token);
    token_client.transfer(&from, to, &transfer_amount);
}
