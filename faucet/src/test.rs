#![cfg(test)]

extern crate std;

use super::{FaucetContract, FaucetContractClient};
use std::println;

use soroban_sdk::testutils::{Address as _, Events, Ledger, LedgerInfo};
use soroban_sdk::token::Interface;
use soroban_sdk::{symbol_short, token, Address, Env, IntoVal, String, Symbol};
use token::Client;

mod token_contract {
    soroban_sdk::contractimport!(file = "../token/soroban_token_contract.optimized.wasm");
}

fn create_custom_token<'a>(
    e: &Env,
    admin: &Address,
    name: &str,
    symbol: &str,
    decimals: &u32,
) -> token_contract::Client<'a> {
    let token_id = &e.register_contract_wasm(None, token_contract::WASM);
    let token = token_contract::Client::new(e, &token_id);
    token.initialize(
        admin,
        decimals,
        &String::from_slice(&e, name),
        &String::from_slice(&e, symbol),
    );
    token
}


#[test]
fn test_request_tokens() {
    let env = Env::default();
    env.mock_all_auths();

    env.budget().reset_unlimited();
    let contract_address = env.register_contract(None, FaucetContract);
    let contract_client = FaucetContractClient::new(&env, &contract_address);
    let admin = Address::generate(&env);
    let user1 = Address::generate(&env);

    let token_xlm = create_custom_token(&env, &admin, "Xlm", "xlm", &7);
    let token_eth = create_custom_token(&env, &admin, "Eth", "eth", &7);

    token_xlm.mint(&contract_address, &1_000_000_0000000);
    token_eth.mint(&contract_address, &1_000_000_0000000);

    contract_client.request_token(&user1, &token_eth.address, &100_0000000);

    assert_eq!(
        token_eth.balance(&user1),
        100_0000000_i128
    );
}