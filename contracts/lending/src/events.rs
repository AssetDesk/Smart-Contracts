use soroban_sdk::{symbol_short, Address, Env, Symbol};

// Events Topics
const ADMIN: Symbol = symbol_short!("admin");
const DEPOSIT: Symbol = symbol_short!("deposit");
const REDEEM: Symbol = symbol_short!("redeem");
const BORROW: Symbol = symbol_short!("borrow");
const REPAY: Symbol = symbol_short!("repay");
const LIQUIDATE: Symbol = symbol_short!("liquidate");

pub(crate) fn set_admin(env: &Env, admin: &Address) {
    let topics = (ADMIN, symbol_short!("set"));
    env.events().publish(topics, admin);
}

pub(crate) fn deposit(env: &Env, user_address: &Address, denom: &Symbol, amount: &u128) {
    let topics = (DEPOSIT, user_address.clone());
    env.events().publish(topics, (denom.clone(), amount.clone()));
}

pub(crate) fn redeem(env: &Env, user_address: &Address, denom: &Symbol, amount: &u128) {
    let topics = (REDEEM, user_address.clone());
    env.events().publish(topics, (denom.clone(), amount.clone()));
}

pub(crate) fn borrow(env: &Env, user_address: &Address, denom: &Symbol, amount: &u128) {
    let topics = (BORROW, user_address.clone());
    env.events().publish(topics, (denom.clone(), amount.clone()));
}

pub(crate) fn repay(env: &Env, user_address: &Address, denom: &Symbol, amount: &u128) {
    let topics = (REPAY, user_address.clone());
    env.events().publish(topics, (denom.clone(), amount.clone()));
}

pub(crate) fn liquidate(env: &Env, user_address: &Address, liquidator: &Address) {
    let topics = (LIQUIDATE, user_address.clone());
    env.events().publish(topics, liquidator.clone());
}