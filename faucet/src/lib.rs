#![no_std]

use soroban_sdk::{contract, contractimpl, token, Address, Env, Symbol, Vec, symbol_short}; // contracterror, panic_with_error, vec


fn move_token(env: &Env, token: &Address, from: &Address, to: &Address, transfer_amount: i128) {
    // new token interface
    let token_client = token::Client::new(&env, &token);
    token_client.transfer(&from, to, &transfer_amount);
}

fn token_balance(env: &Env, token: &Address, user_address: &Address) -> i128 {
    let token_client = token::Client::new(&env, &token);
    token_client.balance(&user_address)
}

fn token_decimals(env: &Env, token: &Address) -> u32 {
    let token_client = token::Client::new(&env, &token);
    token_client.decimals()
}


#[contract]
pub struct FaucetContract;

#[contractimpl]
impl FaucetContract {

    pub fn request_token(env: Env, to_user: Address, token_address: Address, token_amount: i128) {
        to_user.require_auth();

        let user_balance: i128 = token_balance(&env, &token_address, &to_user);
        if user_balance >= 1000_i128 * 10_i128.pow(token_decimals(&env, &token_address)) {
            panic!("Limit");
        }

        move_token(&env, &token_address, &env.current_contract_address(), &to_user, token_amount);

    }
}

mod test;
