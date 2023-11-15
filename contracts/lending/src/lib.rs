#![no_std]

use soroban_sdk::{
    contract, contractimpl, map, symbol_short, token, Address, Env, Map, Symbol, Vec,
}; // contracterror, panic_with_error, vec

use core::ops::{Add, Div, Mul};
use rust_decimal::prelude::{Decimal, MathematicalOps, ToPrimitive};

use crate::storage::*;

#[contract]
pub struct LendingContract;

#[contractimpl]
impl LendingContract {
    pub fn initialize(e: Env, admin: Address, liquidator: Address) {
        if has_admin(&e) {
            panic!("already initialized")
        }
        set_admin(&e, &admin);
        set_liquidator(&e, &liquidator);
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
            .get(&DataKey::UserMMTokenBalance(user_address.clone()))
            .unwrap_or(Map::new(&env))
            .get(denom.clone())
            .unwrap_or(0_u128);

        let new_user_mm_token_balance: u128 =
            user_current_mm_token_balance + deposited_mm_token_amount;

        let mut user_mm_balance_map: Map<Symbol, u128> = env
            .storage()
            .persistent()
            .get(&DataKey::UserMMTokenBalance(user_address.clone()))
            .unwrap_or(Map::new(&env));
        user_mm_balance_map.set(denom.clone(), new_user_mm_token_balance);
        env.storage().persistent().set(
            &DataKey::UserMMTokenBalance(user_address.clone()),
            &user_mm_balance_map,
        );
        env.storage().persistent().bump(
            &DataKey::UserMMTokenBalance(user_address.clone()),
            MONTH_LIFETIME_THRESHOLD,
            MONTH_BUMP_AMOUNT,
        );
    }

    pub fn add_markets(
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

        let reserve_configuration: ReserveConfiguration = ReserveConfiguration {
            denom: denom.clone(),
            loan_to_value_ratio,
            liquidation_threshold,
        };
        let mut reserve_map: Map<Symbol, ReserveConfiguration> = env
            .storage()
            .persistent()
            .get(&DataKey::ReserveConfiguration)
            .unwrap_or(Map::new(&env));
        reserve_map.set(denom.clone(), reserve_configuration);
        env.storage()
            .persistent()
            .set(&DataKey::ReserveConfiguration, &reserve_map);
        env.storage().persistent().bump(
            &DataKey::ReserveConfiguration,
            MONTH_LIFETIME_THRESHOLD,
            MONTH_BUMP_AMOUNT,
        );

        let tokens_interest_rate_model_params: TokenInterestRateModelParams =
            TokenInterestRateModelParams {
                denom: denom.clone(),
                min_interest_rate,
                safe_borrow_max_rate,
                rate_growth_factor,
                optimal_utilization_ratio,
            };
        let mut token_interest_params_map: Map<Symbol, TokenInterestRateModelParams> = env
            .storage()
            .persistent()
            .get(&DataKey::TokensInterestRateModelParams)
            .unwrap_or(Map::new(&env));
        token_interest_params_map.set(denom.clone(), tokens_interest_rate_model_params);
        env.storage().persistent().set(
            &DataKey::TokensInterestRateModelParams,
            &token_interest_params_map,
        );
        env.storage().persistent().bump(
            &DataKey::TokensInterestRateModelParams,
            MONTH_LIFETIME_THRESHOLD,
            MONTH_BUMP_AMOUNT,
        );

        let total_borrow_data: TotalBorrowData = TotalBorrowData {
            denom: denom.clone(),
            total_borrowed_amount: 0_u128,
            expected_annual_interest_income: 0_u128,
            average_interest_rate: 0_u128,
            timestamp: env.ledger().timestamp(),
        };

        let mut total_borrow_map: Map<Symbol, TotalBorrowData> = env
            .storage()
            .persistent()
            .get(&DataKey::TotalBorrowData)
            .unwrap_or(Map::new(&env));
        total_borrow_map.set(denom.clone(), total_borrow_data);
        env.storage()
            .persistent()
            .set(&DataKey::TotalBorrowData, &total_borrow_map);
        env.storage().persistent().bump(
            &DataKey::TotalBorrowData,
            MONTH_LIFETIME_THRESHOLD,
            MONTH_BUMP_AMOUNT,
        );

        let liquidity_index_data: LiquidityIndexData = LiquidityIndexData {
            denom: denom.clone(),
            liquidity_index_ln: 0_u128,
            timestamp: env.ledger().timestamp(),
        };

        let mut liquidity_map: Map<Symbol, LiquidityIndexData> = env
            .storage()
            .persistent()
            .get(&DataKey::LiquidityIndexData)
            .unwrap_or(Map::new(&env));
        liquidity_map.set(denom.clone(), liquidity_index_data);
        env.storage()
            .persistent()
            .set(&DataKey::LiquidityIndexData, &liquidity_map);
        env.storage().persistent().bump(
            &DataKey::LiquidityIndexData,
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

    pub fn toggle_collateral_setting(env: Env, user: Address, denom: Symbol) {
        user.require_auth();

        let use_user_deposit_as_collateral =
            user_deposit_as_collateral(env.clone(), user.clone(), denom.clone());

        if use_user_deposit_as_collateral {
            let user_token_balance: u128 = get_deposit(env.clone(), user.clone(), denom.clone());

            if user_token_balance != 0 {
                let token_decimals = get_token_decimal(env.clone(), denom.clone());

                let price = fetch_price_by_token(env.clone(), denom.clone());

                let user_token_balance_usd =
                    Decimal::from_i128_with_scale(user_token_balance as i128, token_decimals)
                        .mul(Decimal::from_i128_with_scale(price as i128, USD_DECIMALS))
                        .to_u128_with_decimals(USD_DECIMALS)
                        .unwrap();

                let sum_collateral_balance_usd = get_user_collateral_usd(env.clone(), user.clone());

                let sum_borrow_balance_usd = get_user_borrowed_usd(env.clone(), user.clone());

                let user_liquidation_threshold =
                    get_user_liquidation_threshold(env.clone(), user.clone());

                assert!(
                    sum_borrow_balance_usd * HUNDRED_PERCENT / user_liquidation_threshold < sum_collateral_balance_usd - user_token_balance_usd,
                    "The collateral has already using to collateralise the borrowing. Not enough available balance"
                );
            }
        }

        let mut user_deposit_as_collateral_map: Map<Symbol, bool> = env
            .storage()
            .persistent()
            .get(&DataKey::UserDepositAsCollateral(user.clone()))
            .unwrap_or(Map::new(&env));
        user_deposit_as_collateral_map.set(denom.clone(), !use_user_deposit_as_collateral);
        env.storage().persistent().set(
            &DataKey::UserDepositAsCollateral(user.clone()),
            &user_deposit_as_collateral_map,
        );
        env.storage().persistent().bump(
            &DataKey::UserDepositAsCollateral(user.clone()),
            MONTH_LIFETIME_THRESHOLD,
            MONTH_BUMP_AMOUNT,
        );
    }

    pub fn borrow(env: Env, user: Address, denom: Symbol, amount: u128) {
        user.require_auth();

        // let liquidator = get_liquidator(&env);

        // if user == liquidator {
        //     panic!("The liquidator cannot borrow");
        // }

        // let supported_tokens: Vec<Symbol> = get_supported_tokens(env.clone());

        // if !supported_tokens.contains(denom.clone()) {
        //     panic!("There is no such supported token yet");
        // }

        let available_to_borrow_amount: u128 =
            get_available_to_borrow(env.clone(), user.clone(), denom.clone());

        if amount > available_to_borrow_amount {
            panic!("The amount to be borrowed is not available");
        }

        //     assert!(
        //         get_available_liquidity_by_token(env.clone(), denom.clone())
        //             .unwrap()
        //             .u128()
        //             >= amount.u128()
        //     );

        execute_update_liquidity_index_data(env.clone(), denom.clone());

        let user_borrow_amount_with_interest: u128 =
            get_user_borrow_amount_with_interest(env.clone(), user.clone(), denom.clone());

        let user_borrowing_info: UserBorrowingInfo =
            get_user_borrowing_info(env.clone(), user.clone(), denom.clone());

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

        let expected_annual_interest_income: u128 = total_borrow_data
            .expected_annual_interest_income
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

        let mut user_borrow_map: Map<Symbol, UserBorrowingInfo> = env
            .storage()
            .persistent()
            .get(&DataKey::UserBorrowingInfo(user.clone()))
            .unwrap_or(Map::new(&env));
        user_borrow_map.set(denom.clone(), new_user_borrowing_info);
        env.storage()
            .persistent()
            .set(&DataKey::UserBorrowingInfo(user.clone()), &user_borrow_map);
        env.storage().persistent().bump(
            &DataKey::UserBorrowingInfo(user.clone()),
            MONTH_LIFETIME_THRESHOLD,
            MONTH_BUMP_AMOUNT,
        );

        let mut total_borrow_map: Map<Symbol, TotalBorrowData> = env
            .storage()
            .persistent()
            .get(&DataKey::TotalBorrowData)
            .unwrap_or(Map::new(&env));
        total_borrow_map.set(denom.clone(), new_total_borrow_data);
        env.storage()
            .persistent()
            .set(&DataKey::TotalBorrowData, &total_borrow_map);
        env.storage().persistent().bump(
            &DataKey::TotalBorrowData,
            MONTH_LIFETIME_THRESHOLD,
            MONTH_BUMP_AMOUNT,
        );

        move_token(
            &env,
            &get_token_address(env.clone(), denom.clone()),
            &env.current_contract_address(),
            &user,
            amount as i128,
        )
    }

    pub fn redeem(env: Env, user: Address, denom: Symbol, mut amount: u128) {
        user.require_auth();

        // let supported_tokens: Vec<Symbol> = get_supported_tokens(env.clone());

        // if !supported_tokens.contains(denom.clone()) {
        //     panic!("There is no such supported token yet");
        // }

        execute_update_liquidity_index_data(env.clone(), denom.clone());

        let current_balance = get_deposit(env.clone(), user.clone(), denom.clone());

        if amount > current_balance {
            panic!("The account doesn't have enough digital tokens to do withdraw");
        }

        if amount == 0 {
            amount = current_balance;
        }

        let remaining: u128 = current_balance - amount;

        let token_decimals: u32 = get_token_decimal(env.clone(), denom.clone());

        let mm_token_price: u128 = get_mm_token_price(env.clone(), denom.clone());

        let new_user_mm_token_balance: u128 =
            Decimal::from_i128_with_scale(remaining as i128, token_decimals)
                .div(Decimal::from_i128_with_scale(
                    mm_token_price as i128,
                    token_decimals,
                ))
                .to_u128_with_decimals(token_decimals)
                .unwrap();

        let mut user_mm_balance_map: Map<Symbol, u128> = env
            .storage()
            .persistent()
            .get(&DataKey::UserMMTokenBalance(user.clone()))
            .unwrap_or(Map::new(&env));
        user_mm_balance_map.set(denom.clone(), new_user_mm_token_balance);
        env.storage().persistent().set(
            &DataKey::UserMMTokenBalance(user.clone()),
            &user_mm_balance_map,
        );
        env.storage().persistent().bump(
            &DataKey::UserMMTokenBalance(user.clone()),
            MONTH_LIFETIME_THRESHOLD,
            MONTH_BUMP_AMOUNT,
        );

        move_token(
            &env,
            &get_token_address(env.clone(), denom.clone()),
            &env.current_contract_address(),
            &user,
            amount as i128,
        )
    }

    pub fn repay(env: Env, user: Address, repay_token: Symbol, mut repay_amount: u128) {
        user.require_auth();

        let token_address: Address = get_token_address(env.clone(), repay_token.clone());
        move_token(
            &env,
            &token_address,
            &user,
            &env.current_contract_address(),
            repay_amount.clone() as i128,
        );

        // let supported_tokens: Vec<Symbol> = get_supported_tokens(env.clone());

        // if !supported_tokens.contains(repay_token.clone()) {
        //     panic!("There is no such supported token yet");
        // }

        let user_borrowing_info =
            get_user_borrowing_info(env.clone(), user.clone(), repay_token.clone());

        execute_update_liquidity_index_data(env.clone(), repay_token.clone());

        let user_borrow_amount_with_interest =
            get_user_borrow_amount_with_interest(env.clone(), user.clone(), repay_token.clone());

        if repay_amount == 0 {
            repay_amount = user_borrow_amount_with_interest;
        }

        let mut remaining_amount: u128 = 0u128;
        let mut average_interest_rate: u128 = user_borrowing_info.average_interest_rate;
        if repay_amount >= user_borrow_amount_with_interest {
            remaining_amount = repay_amount - user_borrow_amount_with_interest;
            repay_amount = user_borrow_amount_with_interest;
            average_interest_rate = 9_u128;
        }

        let new_user_borrowing_info: UserBorrowingInfo = UserBorrowingInfo {
            borrowed_amount: (user_borrow_amount_with_interest - repay_amount),
            average_interest_rate: average_interest_rate,
            timestamp: env.ledger().timestamp(),
        };

        let total_borrow_data: TotalBorrowData =
            get_total_borrow_data(env.clone(), repay_token.clone());

        let repay_token_decimals: u32 = get_token_decimal(env.clone(), repay_token.clone());

        let expected_annual_interest_income = total_borrow_data.expected_annual_interest_income
            + Decimal::from_i128_with_scale(
            (user_borrow_amount_with_interest - user_borrowing_info.borrowed_amount) as i128,
            repay_token_decimals,
        )
            .mul(Decimal::from_i128_with_scale(
                (user_borrowing_info.average_interest_rate / HUNDRED) as i128,
                INTEREST_RATE_DECIMALS,
            ))
            .to_u128_with_decimals(INTEREST_RATE_DECIMALS)
            .unwrap()
            - Decimal::from_i128_with_scale((repay_amount) as i128, repay_token_decimals)
            .mul(Decimal::from_i128_with_scale(
                (user_borrowing_info.average_interest_rate / HUNDRED) as i128,
                INTEREST_RATE_DECIMALS,
            ))
            .to_u128_with_decimals(INTEREST_RATE_DECIMALS)
            .unwrap();

        let total_borrowed_amount: u128 = total_borrow_data.total_borrowed_amount
            + user_borrow_amount_with_interest
            - user_borrowing_info.borrowed_amount
            - repay_amount;

        let mut total_average_interest_rate: u128 = 0u128;
        if total_borrowed_amount != 0u128 {
            total_average_interest_rate = HUNDRED
                * Decimal::from_i128_with_scale(
                expected_annual_interest_income as i128,
                INTEREST_RATE_DECIMALS,
            )
                .div(Decimal::from_i128_with_scale(
                    total_borrowed_amount as i128,
                    repay_token_decimals,
                ))
                .to_u128_with_decimals(INTEREST_RATE_DECIMALS)
                .unwrap();
        }

        let new_total_borrow_data = TotalBorrowData {
            denom: repay_token.clone(),
            total_borrowed_amount: total_borrowed_amount,
            expected_annual_interest_income: expected_annual_interest_income,
            average_interest_rate: total_average_interest_rate,
            timestamp: env.ledger().timestamp(),
        };

        let mut user_borrow_map: Map<Symbol, UserBorrowingInfo> = env
            .storage()
            .persistent()
            .get(&DataKey::UserBorrowingInfo(user.clone()))
            .unwrap_or(Map::new(&env));
        user_borrow_map.set(repay_token.clone(), new_user_borrowing_info);
        env.storage()
            .persistent()
            .set(&DataKey::UserBorrowingInfo(user.clone()), &user_borrow_map);
        env.storage().persistent().bump(
            &DataKey::UserBorrowingInfo(user.clone()),
            MONTH_LIFETIME_THRESHOLD,
            MONTH_BUMP_AMOUNT,
        );

        let mut total_borrow_map: Map<Symbol, TotalBorrowData> = env
            .storage()
            .persistent()
            .get(&DataKey::TotalBorrowData)
            .unwrap_or(Map::new(&env));
        total_borrow_map.set(repay_token.clone(), new_total_borrow_data);
        env.storage()
            .persistent()
            .set(&DataKey::TotalBorrowData, &total_borrow_map);
        env.storage().persistent().bump(
            &DataKey::TotalBorrowData,
            MONTH_LIFETIME_THRESHOLD,
            MONTH_BUMP_AMOUNT,
        );

        if remaining_amount > 0 {
            move_token(
                &env,
                &token_address,
                &env.current_contract_address(),
                &user,
                remaining_amount as i128,
            );
        }
    }

    pub fn liquidation(env: Env, user: Address) {
        // liquidator only
        let liquidator: Address = get_liquidator(&env);
        liquidator.require_auth();

        let user_utilization_rate = get_user_utilization_rate(env.clone(), user.clone());

        let user_liquidation_threshold: u128 =
            get_user_liquidation_threshold(env.clone(), user.clone());

        assert!(
            user_utilization_rate >= user_liquidation_threshold,
            "User borrowing has not reached the threshold of liquidation"
        );

        for token in get_supported_tokens(env.clone()) {
            execute_update_liquidity_index_data(env.clone(), token.clone());

            let use_user_deposit_as_collateral =
                user_deposit_as_collateral(env.clone(), user.clone(), token.clone());

            let mut user_token_balance = 0u128;
            if use_user_deposit_as_collateral {
                user_token_balance = get_deposit(env.clone(), user.clone(), token.clone());

                let mut user_mm_balance_map: Map<Symbol, u128> = env
                    .storage()
                    .persistent()
                    .get(&DataKey::UserMMTokenBalance(user.clone()))
                    .unwrap_or(Map::new(&env));
                user_mm_balance_map.set(token.clone(), 0_u128);
                env.storage().persistent().set(
                    &DataKey::UserMMTokenBalance(user.clone()),
                    &user_mm_balance_map,
                );
                env.storage().persistent().bump(
                    &DataKey::UserMMTokenBalance(user.clone()),
                    MONTH_LIFETIME_THRESHOLD,
                    MONTH_BUMP_AMOUNT,
                );
            }

            let user_borrow_amount_with_interest =
                get_user_borrow_amount_with_interest(env.clone(), user.clone(), token.clone());

            if user_borrow_amount_with_interest > 0 || user_token_balance > 0 {
                let liquidator_balance =
                    get_deposit(env.clone(), liquidator.clone(), token.clone());

                let token_decimals = get_token_decimal(env.clone(), token.clone());

                if user_borrow_amount_with_interest > 0 {
                    assert!(
                        liquidator_balance >= user_borrow_amount_with_interest,
                        "The liquidator does not have enough deposit balance for liquidation"
                    );

                    let user_borrowing_info =
                        get_user_borrowing_info(env.clone(), user.clone(), token.clone());

                    let new_user_borrowing_info = UserBorrowingInfo {
                        borrowed_amount: 0_u128,
                        average_interest_rate: 0_u128,
                        timestamp: env.ledger().timestamp(),
                    };

                    let total_borrow_data = get_total_borrow_data(env.clone(), token.clone());

                    let expected_annual_interest_income = total_borrow_data
                        .expected_annual_interest_income
                        - Decimal::from_i128_with_scale(
                        (user_borrowing_info.borrowed_amount) as i128,
                        token_decimals,
                    )
                        .mul(Decimal::from_i128_with_scale(
                            (user_borrowing_info.average_interest_rate / HUNDRED) as i128,
                            INTEREST_RATE_DECIMALS,
                        ))
                        .to_u128_with_decimals(INTEREST_RATE_DECIMALS)
                        .unwrap();

                    let total_borrowed_amount = total_borrow_data.total_borrowed_amount
                        - user_borrowing_info.borrowed_amount;

                    let mut total_average_interest_rate = 0u128;
                    if total_borrowed_amount != 0u128 {
                        total_average_interest_rate = HUNDRED
                            * Decimal::from_i128_with_scale(
                            expected_annual_interest_income as i128,
                            INTEREST_RATE_DECIMALS,
                        )
                            .div(Decimal::from_i128_with_scale(
                                total_borrowed_amount as i128,
                                token_decimals,
                            ))
                            .to_u128_with_decimals(INTEREST_RATE_DECIMALS)
                            .unwrap();
                    }

                    let new_total_borrow_data = TotalBorrowData {
                        denom: token.clone(),
                        total_borrowed_amount: total_borrowed_amount,
                        expected_annual_interest_income: expected_annual_interest_income,
                        average_interest_rate: total_average_interest_rate,
                        timestamp: env.ledger().timestamp(),
                    };

                    let mut user_borrow_map: Map<Symbol, UserBorrowingInfo> = env
                        .storage()
                        .persistent()
                        .get(&DataKey::UserBorrowingInfo(user.clone()))
                        .unwrap_or(Map::new(&env));
                    user_borrow_map.set(token.clone(), new_user_borrowing_info);
                    env.storage()
                        .persistent()
                        .set(&DataKey::UserBorrowingInfo(user.clone()), &user_borrow_map);
                    env.storage().persistent().bump(
                        &DataKey::UserBorrowingInfo(user.clone()),
                        MONTH_LIFETIME_THRESHOLD,
                        MONTH_BUMP_AMOUNT,
                    );
                    let mut total_borrow_map: Map<Symbol, TotalBorrowData> = env
                        .storage()
                        .persistent()
                        .get(&DataKey::TotalBorrowData)
                        .unwrap_or(Map::new(&env));
                    total_borrow_map.set(token.clone(), new_total_borrow_data);
                    env.storage()
                        .persistent()
                        .set(&DataKey::TotalBorrowData, &total_borrow_map);
                    env.storage().persistent().bump(
                        &DataKey::TotalBorrowData,
                        MONTH_LIFETIME_THRESHOLD,
                        MONTH_BUMP_AMOUNT,
                    );
                }

                let new_liquidator_token_balance: u128 =
                    liquidator_balance + user_token_balance - user_borrow_amount_with_interest;

                let mm_token_price = get_mm_token_price(env.clone(), token.clone());

                let new_liquidator_mm_token_balance = Decimal::from_i128_with_scale(
                    new_liquidator_token_balance as i128,
                    token_decimals,
                )
                    .div(Decimal::from_i128_with_scale(
                        mm_token_price as i128,
                        token_decimals,
                    ))
                    .to_u128_with_decimals(token_decimals)
                    .unwrap();

                let mut liquidator_mm_balance_map: Map<Symbol, u128> = env
                    .storage()
                    .persistent()
                    .get(&DataKey::UserMMTokenBalance(liquidator.clone()))
                    .unwrap_or(Map::new(&env));
                liquidator_mm_balance_map.set(token.clone(), new_liquidator_mm_token_balance);
                env.storage().persistent().set(
                    &DataKey::UserMMTokenBalance(liquidator.clone()),
                    &liquidator_mm_balance_map,
                );
                env.storage().persistent().bump(
                    &DataKey::UserMMTokenBalance(liquidator.clone()),
                    MONTH_LIFETIME_THRESHOLD,
                    MONTH_BUMP_AMOUNT,
                );
            }
        }
    }

    pub fn get_deposit(env: Env, user: Address, denom: Symbol) -> u128 {
        get_deposit(env, user, denom)
    }

    pub fn get_total_borrow_data(env: Env, denom: Symbol) -> TotalBorrowData {
        get_total_borrow_data(env, denom)
    }

    pub fn get_total_reserves_by_token(env: Env, denom: Symbol) -> u128 {
        get_total_reserves_by_token(env, denom)
    }

    pub fn get_user_deposited_usd(env: Env, user: Address) -> u128 {
        get_user_deposited_usd(env, user)
    }

    pub fn get_mm_token_price(env: Env, denom: Symbol) -> u128 {
        get_mm_token_price(env, denom)
    }

    pub fn get_price(env: Env, denom: Symbol) -> u128 {
        fetch_price_by_token(env, denom)
    }

    pub fn get_liquidity_rate(env: Env, denom: Symbol) -> u128 {
        get_liquidity_rate(env, denom)
    }

    pub fn get_user_borrow_with_interest(env: Env, user: Address, denom: Symbol) -> u128 {
        get_user_borrow_amount_with_interest(env, user, denom)
    }

    pub fn get_user_max_allowed_borrow_usd(env: Env, user: Address) -> u128 {
        get_user_max_allowed_borrow_amount_usd(env, user)
    }

    pub fn get_user_borrowed_usd(env: Env, user: Address) -> u128 {
        get_user_borrowed_usd(env, user)
    }

    pub fn get_user_liquidation_threshold(env: Env, user: Address) -> u128 {
        get_user_liquidation_threshold(env, user)
    }

    pub fn get_user_utilization_rate(env: Env, user: Address) -> u128 {
        get_user_utilization_rate(env, user)
    }
    pub fn get_available_to_borrow(env: Env, user: Address, denom: Symbol) -> u128 {
        get_available_to_borrow(env, user, denom)
    }

    pub fn get_available_to_redeem(env: Env, user: Address, denom: Symbol) -> u128 {
        get_available_to_redeem(env, user, denom)
    }

    pub fn get_user_collateral_usd(env: Env, user: Address) -> u128 {
        get_user_collateral_usd(env, user)
    }

    pub fn get_reserve_configuration(env: Env, denom: Symbol) -> ReserveConfiguration {
        get_reserve_configuration(env, denom)
    }

    pub fn set_reserve_configuration(
        env: Env,
        denom: Symbol,
        loan_to_value_ratio: u128,
        liquidation_threshold: u128,
    ) {
        let admin: Address = get_admin(&env);
        admin.require_auth();

        let supported_tokens: Vec<Symbol> = get_supported_tokens(env.clone());

        if !supported_tokens.contains(denom.clone()) {
            panic!("There is no such supported token yet");
        }

        let mut reserve_map: Map<Symbol, ReserveConfiguration> = env
            .storage()
            .persistent()
            .get(&DataKey::ReserveConfiguration)
            .unwrap_or(Map::new(&env));
        reserve_map.set(
            denom.clone(),
            ReserveConfiguration {
                denom: denom.clone(),
                loan_to_value_ratio,
                liquidation_threshold,
            },
        );
        env.storage()
            .persistent()
            .set(&DataKey::ReserveConfiguration, &reserve_map);
        env.storage().persistent().bump(
            &DataKey::ReserveConfiguration,
            MONTH_LIFETIME_THRESHOLD,
            MONTH_BUMP_AMOUNT,
        );
    }

    pub fn     set_token_interest_rate_params(
        env: Env,
        denom: Symbol,
        min_interest_rate: u128,
        safe_borrow_max_rate: u128,
        rate_growth_factor: u128,
        optimal_utilization_ratio: u128,
    ) {
        let admin: Address = get_admin(&env);
        admin.require_auth();

        let supported_tokens: Vec<Symbol> = get_supported_tokens(env.clone());

        if !supported_tokens.contains(denom.clone()) {
            panic!("There is no such supported token yet");
        }

        let mut token_interest_params_map: Map<Symbol, TokenInterestRateModelParams> = env
            .storage()
            .persistent()
            .get(&DataKey::TokensInterestRateModelParams)
            .unwrap_or(Map::new(&env));
        token_interest_params_map.set(
            denom.clone(),
            TokenInterestRateModelParams {
                denom: denom.clone(),
                min_interest_rate,
                safe_borrow_max_rate,
                rate_growth_factor,
                optimal_utilization_ratio,
            },
        );

        env.storage().persistent().set(
            &DataKey::TokensInterestRateModelParams,
            &token_interest_params_map,
        );
        env.storage().persistent().bump(
            &DataKey::TokensInterestRateModelParams,
            MONTH_LIFETIME_THRESHOLD,
            MONTH_BUMP_AMOUNT,
        );
    }

    pub fn get_interest_rate(env: Env, denom: Symbol) -> u128 {
        get_interest_rate(env, denom)
    }

    pub fn get_user_borrowing_info(env: Env, user: Address, denom: Symbol) -> UserBorrowingInfo {
        get_user_borrowing_info(env, user, denom)
    }

    pub fn user_deposit_as_collateral(env: Env, user: Address, denom: Symbol) -> bool {
        user_deposit_as_collateral(env, user, denom)
    }

    pub fn get_available_liquidity_by_token(env: Env, denom: Symbol) -> u128 {
        get_available_liquidity_by_token(env, denom)
    }

    pub fn get_utilization_rate_by_token(env: Env, denom: Symbol) -> u128 {
        get_utilization_rate_by_token(env, denom)
    }

    pub fn get_total_borrowed_by_token(env: Env, denom: Symbol) -> u128 {
        get_total_borrowed_by_token(env, denom)
    }

    pub fn get_tvl(env: Env) -> u128 {
        let supported_tokens: Vec<Symbol> = get_supported_tokens(env.clone());
        let mut tvl_usd: u128 = 0;
        for token in supported_tokens {
            let token_decimals: u32 = get_token_decimal(env.clone(), token.clone());
            let price: u128 = fetch_price_by_token(env.clone(), token.clone());
            let liquidity: u128 = get_available_liquidity_by_token(env.clone(), token.clone());
            tvl_usd += price * liquidity / 10_u128.pow(token_decimals);
        }
        tvl_usd
    }

    // pub fn GetAllUsersWithBorrows(env: Env) -> Vec<Address> {
    //     get_all_users_with_borrows(env)
    // }
}

mod test;
mod storage;