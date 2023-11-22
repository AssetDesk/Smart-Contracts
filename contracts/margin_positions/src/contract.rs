#![no_std]

use soroban_sdk::{
    contract, contractimpl, map, symbol_short, token, Address, Env, Map, String, Symbol, Vec,
};

use core::ops::{Add, Div, Mul};
use rust_decimal::prelude::{Decimal, MathematicalOps, ToPrimitive};

use crate::storage::*;
use crate::utilities::*;

mod vault_contract {
    soroban_sdk::contractimport!(
        file = "../../target/wasm32-unknown-unknown/release/vault_contract.wasm"
    );
}

#[contract]
pub(crate) struct MarginPositionsContract;

#[contractimpl]
impl MarginPositionsContract {
    pub fn initialize(
        env: Env,
        admin: Address,
        liquidator: Address,
        collateral_vault_contract: Address,
    ) {
        if has_admin(&env) {
            panic!("already initialized")
        }
        set_admin(&env, &admin);
        set_liquidator(&env, &liquidator);
        set_vault_contract(&env, &collateral_vault_contract);
    }

    pub fn deposit(env: Env, user_address: Address, denom: Symbol, deposited_token_amount: u128) {
        user_address.require_auth();

        let token_address: Address = get_token_address(env.clone(), denom.clone());
        move_token(
            &env,
            &token_address,
            &user_address,
            &env.current_contract_address(),
            deposited_token_amount.clone() as i128,
        );

        let user_current_token_balance: u128 = env
            .storage()
            .persistent()
            .get(&DataKey::UserBalance(user_address.clone()))
            .unwrap_or(Map::new(&env))
            .get(denom.clone())
            .unwrap_or(0_u128);

        let new_user_token_balance: u128 = user_current_token_balance + deposited_token_amount;

        let mut user_balance_map: Map<Symbol, u128> = env
            .storage()
            .persistent()
            .get(&DataKey::UserBalance(user_address.clone()))
            .unwrap_or(Map::new(&env));
        user_balance_map.set(denom.clone(), new_user_token_balance);
        env.storage().persistent().set(
            &DataKey::UserBalance(user_address.clone()),
            &user_balance_map,
        );
        env.storage().persistent().bump(
            &DataKey::UserBalance(user_address.clone()),
            MONTH_LIFETIME_THRESHOLD,
            MONTH_BUMP_AMOUNT,
        );

        move_token(
            &env,
            &get_token_address(env.clone(), denom.clone()),
            &env.current_contract_address(),
            &get_vault_contract(&env),
            deposited_token_amount.clone() as i128,
        )
    }

    pub fn redeem(env: Env, user: Address, denom: Symbol, mut amount: u128) {
        user.require_auth();

        // let supported_tokens: Vec<Symbol> = get_supported_tokens(env.clone());

        // if !supported_tokens.contains(denom.clone()) {
        //     panic!("There is no such supported token yet");
        // }

        let current_balance = get_deposit(env.clone(), user.clone(), denom.clone());

        if amount > current_balance {
            panic!("The account doesn't have enough digital tokens redeem");
        }

        if amount == 0 {
            amount = current_balance;
        }

        let remaining: u128 = current_balance.clone() - amount;

        let mut user_balance_map: Map<Symbol, u128> = env
            .storage()
            .persistent()
            .get(&DataKey::UserBalance(user.clone()))
            .unwrap_or(Map::new(&env));
        user_balance_map.set(denom.clone(), remaining.clone());
        env.storage()
            .persistent()
            .set(&DataKey::UserBalance(user.clone()), &user_balance_map);
        env.storage().persistent().bump(
            &DataKey::UserBalance(user.clone()),
            MONTH_LIFETIME_THRESHOLD,
            MONTH_BUMP_AMOUNT,
        );

        // vault cross-contract call to redeem money
        let vault_contract_client = vault_contract::Client::new(&env, &get_vault_contract(&env));
        vault_contract_client.redeem_from_vault_contract_m(
            &user,
            &get_token_address(env.clone(), denom.clone()),
            &amount,
        )
    }

    // pub fn liquidation(env: Env, user: Address) {
    //     // liquidator only
    //     let liquidator: Address = get_liquidator(&env);
    //     liquidator.require_auth();
    //
    //     let user_utilization_rate = get_user_utilization_rate(env.clone(), user.clone());
    //
    //     let user_liquidation_threshold: u128 =
    //         get_user_liquidation_threshold(env.clone(), user.clone());
    //
    //     assert!(
    //         user_utilization_rate >= user_liquidation_threshold,
    //         "User borrowing has not reached the threshold of liquidation"
    //     );
    //
    //     for token in get_supported_tokens(env.clone()) {
    //         execute_update_liquidity_index_data(env.clone(), token.clone());
    //
    //         let use_user_deposit_as_collateral =
    //             user_deposit_as_collateral(env.clone(), user.clone(), token.clone());
    //
    //         let mut user_token_balance = 0u128;
    //         if use_user_deposit_as_collateral {
    //             user_token_balance = get_deposit(env.clone(), user.clone(), token.clone());
    //
    //             let mut user_mm_balance_map: Map<Symbol, u128> = env
    //                 .storage()
    //                 .persistent()
    //                 .get(&DataKey::UserMMTokenBalance(user.clone()))
    //                 .unwrap_or(Map::new(&env));
    //             user_mm_balance_map.set(token.clone(), 0_u128);
    //             env.storage().persistent().set(
    //                 &DataKey::UserMMTokenBalance(user.clone()),
    //                 &user_mm_balance_map,
    //             );
    //             env.storage().persistent().bump(
    //                 &DataKey::UserMMTokenBalance(user.clone()),
    //                 MONTH_LIFETIME_THRESHOLD,
    //                 MONTH_BUMP_AMOUNT,
    //             );
    //         }
    //
    //         let user_borrow_amount_with_interest =
    //             get_user_borrow_amount_with_interest(env.clone(), user.clone(), token.clone());
    //
    //         if user_borrow_amount_with_interest > 0 || user_token_balance > 0 {
    //             let liquidator_balance =
    //                 get_deposit(env.clone(), liquidator.clone(), token.clone());
    //
    //             let token_decimals = get_token_decimal(env.clone(), token.clone());
    //
    //             if user_borrow_amount_with_interest > 0 {
    //                 assert!(
    //                     liquidator_balance >= user_borrow_amount_with_interest,
    //                     "The liquidator does not have enough deposit balance for liquidation"
    //                 );
    //
    //                 let user_borrowing_info =
    //                     get_user_borrowing_info(env.clone(), user.clone(), token.clone());
    //
    //                 let new_user_borrowing_info = UserBorrowingInfo {
    //                     borrowed_amount: 0_u128,
    //                     average_interest_rate: 0_u128,
    //                     timestamp: env.ledger().timestamp(),
    //                 };
    //
    //                 let total_borrow_data = get_total_borrow_data(env.clone(), token.clone());
    //
    //                 let expected_annual_interest_income = total_borrow_data
    //                     .expected_annual_interest_income
    //                     - Decimal::from_i128_with_scale(
    //                     (user_borrowing_info.borrowed_amount) as i128,
    //                     token_decimals,
    //                 )
    //                     .mul(Decimal::from_i128_with_scale(
    //                         (user_borrowing_info.average_interest_rate / HUNDRED) as i128,
    //                         INTEREST_RATE_DECIMALS,
    //                     ))
    //                     .to_u128_with_decimals(INTEREST_RATE_DECIMALS)
    //                     .unwrap();
    //
    //                 let total_borrowed_amount = total_borrow_data.total_borrowed_amount
    //                     - user_borrowing_info.borrowed_amount;
    //
    //                 let mut total_average_interest_rate = 0u128;
    //                 if total_borrowed_amount != 0u128 {
    //                     total_average_interest_rate = HUNDRED
    //                         * Decimal::from_i128_with_scale(
    //                         expected_annual_interest_income as i128,
    //                         INTEREST_RATE_DECIMALS,
    //                     )
    //                         .div(Decimal::from_i128_with_scale(
    //                             total_borrowed_amount as i128,
    //                             token_decimals,
    //                         ))
    //                         .to_u128_with_decimals(INTEREST_RATE_DECIMALS)
    //                         .unwrap();
    //                 }
    //
    //                 let new_total_borrow_data = TotalBorrowData {
    //                     denom: token.clone(),
    //                     total_borrowed_amount: total_borrowed_amount,
    //                     expected_annual_interest_income: expected_annual_interest_income,
    //                     average_interest_rate: total_average_interest_rate,
    //                     timestamp: env.ledger().timestamp(),
    //                 };
    //
    //                 let mut user_borrow_map: Map<Symbol, UserBorrowingInfo> = env
    //                     .storage()
    //                     .persistent()
    //                     .get(&DataKey::UserBorrowingInfo(user.clone()))
    //                     .unwrap_or(Map::new(&env));
    //                 user_borrow_map.set(token.clone(), new_user_borrowing_info);
    //                 env.storage()
    //                     .persistent()
    //                     .set(&DataKey::UserBorrowingInfo(user.clone()), &user_borrow_map);
    //                 env.storage().persistent().bump(
    //                     &DataKey::UserBorrowingInfo(user.clone()),
    //                     MONTH_LIFETIME_THRESHOLD,
    //                     MONTH_BUMP_AMOUNT,
    //                 );
    //                 let mut total_borrow_map: Map<Symbol, TotalBorrowData> = env
    //                     .storage()
    //                     .persistent()
    //                     .get(&DataKey::TotalBorrowData)
    //                     .unwrap_or(Map::new(&env));
    //                 total_borrow_map.set(token.clone(), new_total_borrow_data);
    //                 env.storage()
    //                     .persistent()
    //                     .set(&DataKey::TotalBorrowData, &total_borrow_map);
    //                 env.storage().persistent().bump(
    //                     &DataKey::TotalBorrowData,
    //                     MONTH_LIFETIME_THRESHOLD,
    //                     MONTH_BUMP_AMOUNT,
    //                 );
    //             }
    //
    //             let new_liquidator_token_balance: u128 =
    //                 liquidator_balance + user_token_balance - user_borrow_amount_with_interest;
    //
    //             let mm_token_price = get_mm_token_price(env.clone(), token.clone());
    //
    //             let new_liquidator_mm_token_balance = Decimal::from_i128_with_scale(
    //                 new_liquidator_token_balance as i128,
    //                 token_decimals,
    //             )
    //                 .div(Decimal::from_i128_with_scale(
    //                     mm_token_price as i128,
    //                     token_decimals,
    //                 ))
    //                 .to_u128_with_decimals(token_decimals)
    //                 .unwrap();
    //
    //             let mut liquidator_mm_balance_map: Map<Symbol, u128> = env
    //                 .storage()
    //                 .persistent()
    //                 .get(&DataKey::UserMMTokenBalance(liquidator.clone()))
    //                 .unwrap_or(Map::new(&env));
    //             liquidator_mm_balance_map.set(token.clone(), new_liquidator_mm_token_balance);
    //             env.storage().persistent().set(
    //                 &DataKey::UserMMTokenBalance(liquidator.clone()),
    //                 &liquidator_mm_balance_map,
    //             );
    //             env.storage().persistent().bump(
    //                 &DataKey::UserMMTokenBalance(liquidator.clone()),
    //                 MONTH_LIFETIME_THRESHOLD,
    //                 MONTH_BUMP_AMOUNT,
    //             );
    //         }
    //     }
    // }

    pub fn add_markets(env: Env, denom: Symbol, address: Address, name: Symbol, decimals: u32) {
        // Admin only
        let admin: Address = get_admin(&env);
        admin.require_auth();

        let mut supported_tokens: Vec<Symbol> = get_supported_tokens(env.clone());
        if supported_tokens.contains(denom.clone()) {
            panic!("There already exists such a supported token");
        }

        supported_tokens.push_back(denom.clone());
        env.storage()
            .persistent()
            .set(&DataKey::SupportedTokensList, &supported_tokens);
        env.storage().persistent().bump(
            &DataKey::SupportedTokensList,
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
            .get(&DataKey::SupportedTokensInfo)
            .unwrap_or(Map::new(&env));
        supported_tokens_info.set(denom.clone(), token_info);
        env.storage()
            .persistent()
            .set(&DataKey::SupportedTokensInfo, &supported_tokens_info);
        env.storage().persistent().bump(
            &DataKey::SupportedTokensInfo,
            MONTH_LIFETIME_THRESHOLD,
            MONTH_BUMP_AMOUNT,
        );
    }

    pub fn update_price(env: Env, denom: Symbol, price: u128) {
        // Admin only
        let admin: Address = get_admin(&env);
        admin.require_auth();

        let mut prices: Map<Symbol, u128> = env
            .storage()
            .persistent()
            .get(&DataKey::Prices)
            .unwrap_or(Map::new(&env));
        prices.set(denom.clone(), price);
        env.storage().persistent().set(&DataKey::Prices, &prices);
        env.storage().persistent().bump(
            &DataKey::Prices,
            MONTH_LIFETIME_THRESHOLD,
            MONTH_BUMP_AMOUNT,
        );
    }

    pub fn get_deposit(env: Env, user: Address, denom: Symbol) -> u128 {
        get_deposit(env, user, denom)
    }

    pub fn get_price(env: Env, denom: Symbol) -> u128 {
        fetch_price_by_token(env, denom)
    }

    pub fn get_available_to_redeem(env: Env, user: Address, denom: Symbol) -> u128 {
        get_available_to_redeem(env, user, denom)
    }

    pub fn set_vault_contract(env: Env, vault_contract: Address) {
        set_vault_contract(&env, &vault_contract)
    }

    pub fn get_vault_contract(env: Env) -> Address {
        get_vault_contract(&env)
    }
}
