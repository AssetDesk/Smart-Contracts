#![no_std]

use core::ptr::addr_of;
use crate::storage::*;
use soroban_sdk::{contract, contractimpl, Address, Env};

#[contract]
pub struct VaultContract;

#[contractimpl]
impl VaultContract {
    // Initializes the contract with the specified admin, lending_contract margin_contract and addresses.
    pub fn initialize(
        env: Env,
        lending_contract: Address,
        margin_contract: Address,
        admin: Address,
    ) {
        if has_admin(&env) {
            panic!("already initialized")
        }

        set_admin(&env, &admin);
        set_lending_contract(&env, &lending_contract);
        set_margin_contract(&env, &margin_contract);
    }

    pub fn set_lending_contract(env: Env, lending_contract: Address) {
        // Admin only
        let admin: Address = get_admin(&env);
        admin.require_auth();

        set_lending_contract(&env, &lending_contract);
    }

    pub fn get_lending_contract(env: Env) -> Address {
        get_lending_contract(&env)
    }


    pub fn set_margin_positions_contract(env: Env, margin_contract: Address) {
        // Admin only
        let admin: Address = get_admin(&env);
        admin.require_auth();

        set_margin_contract(&env, &margin_contract);
    }

    pub fn redeem_from_vault_contract(
        env: Env,
        user_address: Address,
        token_address: Address,
        amount: u128,
    ) {
        // Admin only
        let lending_contract: Address = get_lending_contract(&env);
        lending_contract.require_auth();

        move_token(
            &env,
            &token_address,
            &env.current_contract_address(),
            &user_address,
            amount as i128,
        )
    }


    // // redeem_from_vault_contract_margin - for margin positions contract
    // pub fn redeem_from_vault_contract_m(
    //     env: Env,
    //     user_address: Address,
    //     token_address: Address,
    //     amount: u128,
    // ) {
    //     // Admin only
    //     let margin_positions_contract: Address = get_margin_contract(&env);
    //     margin_positions_contract.require_auth();
    //
    //     move_token(
    //         &env,
    //         &token_address,
    //         &env.current_contract_address(),
    //         &user_address,
    //         amount as i128,
    //     )
    // }
    //
    pub fn borrow_from_vault_contract(
        env: Env,
        user_address: Address,
        token_address: Address,
        amount: u128,
    ) {
        // Admin only
        let lending_contract: Address = get_lending_contract(&env);
        lending_contract.require_auth();

        move_token(
            &env,
            &token_address,
            &env.current_contract_address(),
            &user_address,
            amount as i128,
        )
    }
}

mod storage;
#[cfg(test)]
mod test;
