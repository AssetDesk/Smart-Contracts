import SorobanClient from 'soroban-client';
import { Address, xdr, ScInt, scValToNative } from 'soroban-client';
import dotenv from 'dotenv';

dotenv.config();

const rpc_url = process.env.RPC_URL;

const contract_address = process.env.CONTRACT_ADDRESS;
const admin = process.env.ADMIN;
const admin_secret = process.env.ADMIN_SECRET;

const liquidator = process.env.LIQUIDATOR;
const liq_secret = process.env.LIQ_SECRET;

const user1 = process.env.USER1;
const user1_secret = process.env.USER1_SECRET;

const xlm_address = process.env.XLM;
const tokenA = process.env.ATK;
const tokenB = process.env.BTK;

// Configure SorobanClient to talk to the soroban-rpc
const server = new SorobanClient.Server(
    rpc_url, { allowHttp: true }
  );

async function tx_sim_with_fee(contract_address, func_name, args) {
    const account = await server.getAccount(admin);
    let fee = 100;
    const contract = new SorobanClient.Contract(contract_address);
    let transaction = new SorobanClient.TransactionBuilder(account, {
        fee,
        networkPassphrase: SorobanClient.Networks.FUTURENET,
        })
        .addOperation(contract.call(func_name, ...args))
        .setTimeout(30)
        .build();
    // console.log(transaction);
    let response = await server.simulateTransaction(transaction);
    // console.log(`--> ${func_name} cost:`, response.cost);
    if (!response.transactionData) {
        console.log(response);
    }
    // console.log(response);
    const readOnly = response.transactionData._data._attributes.resources._attributes.footprint._attributes.readOnly;
    const n_reads = readOnly.length;
    const readWrite =  response.transactionData._data._attributes.resources._attributes.footprint._attributes.readWrite;
    const n_writes = readWrite.length;
    // console.log(`    Reads: ${n_reads}, Writes: ${n_writes}`);
    // console.log("======================================================");

    const tx_result = scValToNative(response.result.retval);
    fee = Number(response.minResourceFee);
    return {tx_result, fee};
}

async function tx_send(func_name, user_address, user_secret, args) {
    const account = await server.getAccount(user_address);

    let {tx_result, fee} = await tx_sim_with_fee(
        contract_address,
        func_name,
        args
        );
    // console.log(tx_result, fee);
    console.log("--> Transaction fee :", fee);

    const contract = new SorobanClient.Contract(contract_address);
    let transaction = new SorobanClient.TransactionBuilder(account, {
        fee,
        networkPassphrase: SorobanClient.Networks.FUTURENET,
        })
        .addOperation(contract.call(func_name, ...args))
        .setTimeout(30)
        .build();

    transaction = await server.prepareTransaction(transaction);
    // console.log(transaction);
    // process.exit(1)

    const sourceKeypair = SorobanClient.Keypair.fromSecret(user_secret);
    transaction.sign(sourceKeypair);

    let response = await server.sendTransaction(transaction);
    let tx_hash = response.hash;
    console.log('Response:', JSON.stringify(response, null, 2));
    while (response.status != "SUCCESS") {
        console.log(`  Waiting... ${response.status}`);
        if (response.status === "ERROR") {
            console.log(response);
            console.log('Transaction ERROR');
            process.exit(1);
        }
        if (response.status === "FAILED") {
            console.log(response);
            console.log('Transaction FAILED');
            process.exit(1);
        }
        // Wait 1 seconds
        await new Promise(resolve => setTimeout(resolve, 1000));
        // See if the transaction is complete
        response = await server.getTransaction(tx_hash);
        }
    console.log('  Transaction status:', response.status);
    // const result = xdr.TransactionResult.fromXDR(response.resultXdr, 'base64');
    return tx_result;
}

async function GetPrice(token) {
    const func_name = "GetPrice";
    const args = [
        xdr.ScVal.scvSymbol(token),
    ];
    const data = await tx_sim_with_fee(contract_address, func_name, args);
    // console.log(data);
    return data.tx_result;
}

async function GetUserDepositedUsd(user_address) {
    const func_name = "GetUserDepositedUsd";
    const args = [
        new SorobanClient.Address(user_address).toScVal(),
    ];
    const data = await tx_sim_with_fee(contract_address, func_name, args);
    // console.log(data);
    return data.tx_result;
}

async function GetAvailableToBorrow(user_address, token) {
    const func_name = "GetAvailableToBorrow";
    const args = [
        new SorobanClient.Address(user_address).toScVal(),
        xdr.ScVal.scvSymbol(token),
    ];
    const data = await tx_sim_with_fee(contract_address, func_name, args);
    // console.log(data);
    return data.tx_result;
}

async function UpdatePrice(token, price) {
    const func_name = "UpdatePrice";
    const args = [
        xdr.ScVal.scvSymbol(token),
        new SorobanClient.ScInt(price).toU128(),
    ];
    // const data = await tx_sim_with_fee(contract_address, func_name, args);
    // console.log(data);
    await tx_send(func_name, admin, admin_secret, args);
}

async function AddMarkets(token, token_address, decimals) {
    const func_name = "AddMarkets";
    const args = [
        xdr.ScVal.scvSymbol(token),
        new SorobanClient.Contract(token_address).address().toScVal(),
        xdr.ScVal.scvSymbol(token),
        xdr.ScVal.scvU32(decimals),
        new SorobanClient.ScInt(75_00000).toU128(),
        new SorobanClient.ScInt(80_00000).toU128(),
        new SorobanClient.ScInt(5_00000_000000_000000n).toU128(),
        new SorobanClient.ScInt(30_00000_000000_000000n).toU128(),
        new SorobanClient.ScInt(70_00000_000000_000000n).toU128(),
        new SorobanClient.ScInt(80_00000).toU128(),
    ];
    // const data = await tx_sim_with_fee(contract_address, func_name, args);
    // console.log(data);
    await tx_send(func_name, admin, admin_secret, args);
}

async function Deposit(user_address, user_secret, token, amount) {
    const func_name = "Deposit";
    const args = [
        new SorobanClient.Address(user_address).toScVal(),
        xdr.ScVal.scvSymbol(token),
        new SorobanClient.ScInt(amount).toU128(),
    ];
    // const data = await tx_sim_with_fee(contract_address, func_name, args);
    // console.log(data);
    await tx_send(func_name, user_address, user_secret, args);
}

async function ToggleCollateralSetting(user_address, user_secret, token) {
    const func_name = "ToggleCollateralSetting";
    const args = [
        new SorobanClient.Address(user_address).toScVal(),
        xdr.ScVal.scvSymbol(token),
    ];
    // const data = await tx_sim_with_fee(contract_address, func_name, args);
    // console.log(data);
    await tx_send(func_name, user_address, user_secret, args);
}

async function Borrow(user_address, user_secret, token, amount) {
    const func_name = "Borrow";
    const args = [
        new SorobanClient.Address(user_address).toScVal(),
        xdr.ScVal.scvSymbol(token),
        new SorobanClient.ScInt(amount).toU128(),
    ];
    // const data = await tx_sim_with_fee(contract_address, func_name, args);
    // console.log(data);
    await tx_send(func_name, user_address, user_secret, args);
}

export async function token_balance(token_address, user_address) {
    const func_name = "balance";
    const args = [
        new SorobanClient.Address(user_address).toScVal(),
    ];
    const data = await tx_sim_with_fee(token_address, func_name, args);
    return data.tx_result;
  }


export async function total_value_locked(supported_tokens_list) {
    let token_addresses = {
        "xlm": xlm_address,
        "atk": tokenA,
        "btk": tokenB
    };
    let token_decimals = {
        "xlm": 10_000_000n,
        "atk": 10_000_000n,
        "btk": 10_000_000n
    };
    let tvl = 0n;

    for (const token of supported_tokens_list) {
        const token_price = await GetPrice(token);
        const token_tvl = await token_balance(token_addresses[token], contract_address);
        tvl += token_price * token_tvl / token_decimals[token];
    }

    return tvl;
}

// ================= Main flow ================= 
console.log("========== Start ==========");
const xlm_price = await GetPrice("xlm");
const atk_price = await GetPrice("atk");
console.log(" xlm price:", xlm_price, Number.parseFloat(10000n * xlm_price / 100_000_000n) / 10000);
console.log(" atk price:", atk_price, Number.parseFloat(10000n * atk_price / 100_000_000n) / 10000);

let tvl_decimal8 = await total_value_locked(["xlm", "atk"]);
let tvl = Number.parseFloat(100n * tvl_decimal8 / 100_000_000n) / 100;
console.log(`Total Value Locked: ${tvl} USD`)

let admin_atk = await token_balance(tokenA, admin);
let admin_xlm = await token_balance(xlm_address, admin);
let admin_deposit = await GetUserDepositedUsd(admin);
let admin_atk_may_borrow = await GetAvailableToBorrow(admin, "atk");
console.log("Admin xlm balance:", admin_xlm, Number.parseFloat(10000n * admin_xlm / 10_000_000n) / 10000);
console.log("      atk balance:", admin_atk, Number.parseFloat(10000n * admin_atk / 10_000_000n) / 10000);
console.log("      deposit usd:", admin_deposit, Number.parseFloat(10000n * admin_deposit / 100_000_000n) / 10000);
console.log("       borrow atk:", admin_atk_may_borrow, Number.parseFloat(10000n * admin_atk_may_borrow / 10_000_000n) / 10000);

// await UpdatePrice("xlm", 11_360_000n); // 0.1136 USD
// await UpdatePrice("atk", 100_000_000n); // 1 USD

// await AddMarkets("xlm", xlm_address, 7);
// await Deposit(admin, admin_secret, "xlm", 1000_0000000n);
// await AddMarkets("atk", tokenA, 7);
// await Deposit(admin, admin_secret, "atk", 1000_0000000n);

// await Deposit(user1, user1_secret, "xlm", 1000_0000000n);
// await ToggleCollateralSetting(user1, user1_secret, "xlm");
// await Borrow(user1, user1_secret, "atk", 50n * 10_000_000n); // 50 atk = 50 usd

let user1_xlm = await token_balance(xlm_address, user1);
let user1_atk = await token_balance(tokenA, user1);
let user1_deposit = await GetUserDepositedUsd(user1);
let user1_atk_may_borrow = await GetAvailableToBorrow(user1, "atk");
console.log("User1 xlm balance:", user1_xlm, Number.parseFloat(10000n * user1_xlm / 10_000_000n) / 10000);
console.log("      atk balance:", user1_atk, Number.parseFloat(10000n * user1_atk / 10_000_000n) / 10000);
console.log("      deposit usd:", user1_deposit, Number.parseFloat(10000n * user1_deposit / 100_000_000n) / 10000);
console.log("       borrow atk:", user1_atk_may_borrow, Number.parseFloat(10000n * user1_atk_may_borrow / 10_000_000n) / 10000);

process.exit(0);
