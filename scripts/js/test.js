import SorobanClient from 'soroban-client';
import { Address, xdr, ScInt, scValToNative } from 'soroban-client';
import dotenv from 'dotenv';

dotenv.config();

const rpc_url = process.env.RPC_URL;

const contract_address = process.env.CONTRACT_ADDRESS;
// const contract_address = "CDSZ45SXDU5ZIBNYL6VB3WC24DT7RJFP5ZQZPOOGNF557FW5XCHKNXUN";
const admin = process.env.ADMIN;
const admin_secret = process.env.ADMIN_SECRET;

const liquidator = process.env.LIQUIDATOR;
const liq_secret = process.env.LIQ_SECRET;

const user1 = process.env.USER1;
const user1_secret = process.env.USER1_SECRET;

const xlm_address = process.env.XLM;
const tokenA = process.env.ATK;
const tokenB = process.env.BTK;
const USDC = process.env.USDC;
const ETH = process.env.ETH;
const FAUCET = process.env.FAUCET;

// Configure SorobanClient to talk to the soroban-rpc
const server = new SorobanClient.Server(
    rpc_url, { allowHttp: true }
  );

function sorobanBill(sim) {
    // const sim = JSON.parse(file)
    // console.log(sim);
    const events = sim.events.map((event) => {
        // const e = xdr.DiagnosticEvent.fromXDR(event, 'base64')

        if (event.event().type().name === 'diagnostic')
            return 0

        return event.toXDR().length
    })

    const events_and_return_value_size = (
        events.reduce((accumulator, currentValue) => accumulator + currentValue, 0) // events
        // + Buffer.from(sim.results[0].xdr, 'base64').length // return value size
    )
    
    // const sorobanTransactionData = xdr.SorobanTransactionData.fromXDR(sim.result.transactionData, 'base64')
    const sorobanTransactionData = sim.transactionData._data._attributes;
    console.log(sorobanTransactionData.resources);
    // process.exit(0)
    console.log({
        CPU_instructions: Number(sim.cost.cpuInsns),
        RAM: Number(sim.cost.memBytes),
        // ledger_entry_reads: sorobanTransactionData.resources._attributes.footprint.readOnly.length,
        // ledger_entry_writes: sorobanTransactionData.resources._attributes.footprint.readWrite.length,
        // transaction_size: 0,
        ledger_write_bytes: sorobanTransactionData.resources._attributes.writeBytes,
        ledger_read_bytes: sorobanTransactionData.resources._attributes.readBytes,
        events_and_return_value_size,
        // ledger_entry_size: 0
    })
}

async function tx_sim_with_fee(contract_address, func_name, args, user = admin, first_time = false) {
    const account = await server.getAccount(user);
    let fee = 2_000_000;
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
    // if (func_name == "Borrow") {
    //     sorobanBill(response);
    // }
    console.log(`--> ${func_name} cost:`, response.cost);
    if (!response.transactionData) {
        console.log(response);
        // console.log(response.events);
    }
    // console.log(response);
    
    const readOnly = response.transactionData._data._attributes.resources._attributes.footprint._attributes.readOnly;
    const n_reads = readOnly.length;
    const readWrite =  response.transactionData._data._attributes.resources._attributes.footprint._attributes.readWrite;
    const n_writes = readWrite.length;
    console.log(`    Reads: ${n_reads}, Writes: ${n_writes}`);
    console.log("======================================================");

    // console.log(response.transactionData._data._attributes.resources);

    if (first_time) {
        // new account
        // response.cost.cpuInsns = String(Math.round(Number(response.cost.cpuInsns) * 1.05));
        // response.cost.memBytes = String(Math.round(Number(response.cost.memBytes) * 1.05));
        // response.minResourceFee = String(Math.round(Number(response.minResourceFee) *1.05));
        // response.transactionData._data._attributes.resources._attributes.instructions = Math.round(Number(response.transactionData._data._attributes.resources._attributes.instructions) * 1.05);
        // response.transactionData._data._attributes.resources._attributes.readBytes = Math.round(Number(response.transactionData._data._attributes.resources._attributes.readBytes) + 52);
        // response.transactionData._data._attributes.resources._attributes.writeBytes = Math.round(Number(response.transactionData._data._attributes.resources._attributes.writeBytes) + 52);

        // console.log(`--> ${func_name} inflated cost:`, response.cost);
    }

    // process.exit(0);

    const tx_result = scValToNative(response.result.retval);
    fee = Number(response.minResourceFee);
    return {tx_result, fee};
}
async function tx_send(func_name, user_address, user_secret, args, first_time = false) {
    const account = await server.getAccount(user_address);

    let {tx_result, fee} = await tx_sim_with_fee(
        contract_address,
        func_name,
        args,
        user_address,
        first_time,
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

    const sourceKeypair = SorobanClient.Keypair.fromSecret(user_secret);
    transaction.sign(sourceKeypair);
    
    // console.log(transaction.toXDR("base64"));
    // process.exit(1)

    let response = await server.sendTransaction(transaction);
    let tx_hash = response.hash;
    console.log('Response:', JSON.stringify(response, null, 2));
    while (response.status != "SUCCESS") {
        console.log(`  Waiting... ${response.status}`);
        if (response.status === "ERROR") {
            console.log(response);
            console.log(response.errorResult._attributes.result);
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

async function GetUserBorrowedUsd(user_address) {
    const func_name = "GetUserBorrowedUsd";
    const args = [
        new SorobanClient.Address(user_address).toScVal(),
    ];
    const data = await tx_sim_with_fee(contract_address, func_name, args);
    // console.log(data);
    return data.tx_result;
}

async function GetDeposit(user_address, token) {
    const func_name = "GetDeposit";
    const args = [
        new SorobanClient.Address(user_address).toScVal(),
        xdr.ScVal.scvSymbol(token),
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

async function GetUserMaxAllowedBorrowAmountUsd(user_address) {
    const func_name = "GetUserMaxAllowedBorrowAmountUsd";
    const args = [
        new SorobanClient.Address(user_address).toScVal(),
    ];
    const data = await tx_sim_with_fee(contract_address, func_name, args);
    // console.log(data);
    return data.tx_result;
}

async function GetUserBorrowingInfo(user_address, token) {
    const func_name = "GetUserBorrowingInfo";
    const args = [
        new SorobanClient.Address(user_address).toScVal(),
        xdr.ScVal.scvSymbol(token),
    ];
    const data = await tx_sim_with_fee(contract_address, func_name, args);
    // console.log(data);
    return data.tx_result;
}

async function GetAvailableToRedeem(user_address, token) {
    const func_name = "GetAvailableToRedeem";
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

async function Faucet(user_address, user_secret, token_address, token_amount) {
    const func_name = "request_token";
    const args = [
        new SorobanClient.Address(user_address).toScVal(),
        new SorobanClient.Contract(token_address).address().toScVal(),
        new SorobanClient.ScInt(token_amount).toI128(),
    ];

    await tx_send(func_name, user_address, user_secret, args);
}

async function AddMarkets(token, token_address, decimals) {

    let min_interest_rate;
    let safe_borrow_max_rate;
    let rate_growth_factor;
    let optimal_utilization_ratio;

    switch (token) {
        case "xlm":
          min_interest_rate = 5_000000_000000_000000n;
          safe_borrow_max_rate = 40_000000_000000_000000n;
          rate_growth_factor = 70_000000_000000_000000n;
          optimal_utilization_ratio = 80_00000;
          break;
        case "usdc":
          min_interest_rate = 5_000000_000000_000000n;
          safe_borrow_max_rate = 20_000000_000000_000000n;
          rate_growth_factor = 100_000000_000000_000000n;
          optimal_utilization_ratio = 80_00000;
          break;
        case "eth":
          min_interest_rate = 5_000000_000000_000000n;
          safe_borrow_max_rate = 50_000000_000000_000000n;
          rate_growth_factor = 60_000000_000000_000000n;
          optimal_utilization_ratio = 80_00000;
          break;
        default:
          min_interest_rate = 5_000000_000000_000000n;
          safe_borrow_max_rate = 50_000000_000000_000000n;
          rate_growth_factor = 60_000000_000000_000000n;
          optimal_utilization_ratio = 80_00000;
      }

    const func_name = "AddMarkets";
    const args = [
        xdr.ScVal.scvSymbol(token),
        new SorobanClient.Contract(token_address).address().toScVal(),
        xdr.ScVal.scvSymbol(token),
        xdr.ScVal.scvU32(decimals),
        new SorobanClient.ScInt(75_00000).toU128(),
        new SorobanClient.ScInt(80_00000).toU128(),
        new SorobanClient.ScInt(min_interest_rate).toU128(),
        new SorobanClient.ScInt(safe_borrow_max_rate).toU128(),
        new SorobanClient.ScInt(rate_growth_factor).toU128(),
        new SorobanClient.ScInt(optimal_utilization_ratio).toU128(),
    ];
    // const data = await tx_sim_with_fee(contract_address, func_name, args);
    // console.log(data);
    await tx_send(func_name, admin, admin_secret, args);
}

async function SetTokenInterestRateModelParams(
    denom,
    min_interest_rate,
    safe_borrow_max_rate,
    rate_growth_factor,
    optimal_utilization_ratio
) {
    const func_name = "SetTokenInterestRateModelParams";
    const args = [
        xdr.ScVal.scvSymbol(denom),
        new SorobanClient.ScInt(min_interest_rate).toU128(),
        new SorobanClient.ScInt(safe_borrow_max_rate).toU128(),
        new SorobanClient.ScInt(rate_growth_factor).toU128(),
        new SorobanClient.ScInt(optimal_utilization_ratio).toU128(),
    ];
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
    let first_time = false;
    let user_balance = token_balance(xlm_address, user_address);
    if (user_balance = 10_000_0000000) {
        first_time = true;
    }
    await tx_send(func_name, user_address, user_secret, args, first_time);
}

async function Redeem(user_address, user_secret, token, amount) {
    const func_name = "Redeem";
    const args = [
        new SorobanClient.Address(user_address).toScVal(),
        xdr.ScVal.scvSymbol(token),
        new SorobanClient.ScInt(amount).toU128(),
    ];
    await tx_send(func_name, user_address, user_secret, args);
}

async function Repay(user_address, user_secret, token, amount) {
    const func_name = "Repay";
    const args = [
        new SorobanClient.Address(user_address).toScVal(),
        xdr.ScVal.scvSymbol(token),
        new SorobanClient.ScInt(amount).toU128(),
    ];
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

async function GetInterestRate(token) {
    const func_name = "GetInterestRate";
    const args = [
        xdr.ScVal.scvSymbol(token),
    ];

    let data = await tx_sim_with_fee(contract_address, func_name, args);
    return data.tx_result;
}

async function GetLiquidityRate(token) {
    const func_name = "GetLiquidityRate";
    const args = [
        xdr.ScVal.scvSymbol(token),
    ];

    let data = await tx_sim_with_fee(contract_address, func_name, args);
    return data.tx_result;
}

export async function token_balance(token_address, user_address) {
    const func_name = "balance";
    const args = [
        new SorobanClient.Address(user_address).toScVal(),
    ];
    const data = await tx_sim_with_fee(token_address, func_name, args);
    return data.tx_result;
  }

// Old function, do not use, for tests only
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

export async function GetTVL() {
    const func_name = "GetTVL";
    const args = [];
    const data = await tx_sim_with_fee(contract_address, func_name, args);
    return data.tx_result;
}

// Fetch the current XLM and ETH price in USD
async function fetchCryptoPrices() {
    const response = await fetch('https://api.coingecko.com/api/v3/coins/markets?vs_currency=usd');
    const data = await response.json();
  
    // Get the XLM and ETH prices from the data
    const xlmPrice = data.find((coin) => coin.id === 'stellar').current_price;
    const ethPrice = data.find((coin) => coin.id === 'ethereum').current_price;
  
    // Return the prices
    return {
      xlmPrice,
      ethPrice,
    };
  }

// ================= Main flow ================= 
// let user2 = "GBWBSCUYRLOK3OCAICZEDJBHMVUWSDNS2R6ABK6Z55R76P4UKXFIZQSZ"; // Vadym
let user2 = "GBYBH5RAWC5GHKVUU6DKQG43UTGCEI2RXY5YXT3H2XIUR6OB4N32U2X6"; // Yura

const donor_01 = "GARLUGHA3CXK5KDFUGKZAKTXNUYSPE6LPQMRLJUF3IQ4FAGVNVMB6YAZ";
const donor_01_s = "SCV7FJGSDIWXCFHYME2BNJEDRFNQJIJ3V4U2L52MLCHL3PHGWMQRQHSB";
const donor_02 = "GAZFD6ZBFVZAWYWIVKKYGWU7B7Q4FGUEJXQ6HWVZGGQXSOQOT3SGH43P";
const donor_02_s = "SBJYBCLGEZEAAG5WVM5I4O6ZYYPDEONMRTROO5LQKVB4TKQR5ONLMH35";
const donor_03 = "GAQURH6FXIT5XNJJ4E6RY3FHYQL7MD34O2PGSQZGNUBJSFWSYQF4GRXW";
const donor_03_s = "SASBCO3DF6E6KJQONDR3W75JIFEWCK654K3GUKW6QGXP6OD6KS5GCBL5";
const donor_04 = "GCKM3K46IYWKWKDD5LSELY7KIN2JQEKAVE2JESHZ5FETBNLZSLOWXGUO";
const donor_04_s = "SAEQ6VKJNKVQXB3TQUZLCBC3TP2NLEX7O7KZA46LPNF76X2L5J4FX7LA";
const donor_05 = "GC4S7QLQSWIJ3HZDRSXQGF4NZU6YYLDM7IQNRUJ7QXAFKR6NJFTFLUNZ";
const donor_05_s = "SDFIAHEV5YX64IGGFRJY7AQVJK4QF2NFRPTUK5FFIG24CJIMGH4RKRQP";
const donor_06 = "GAPHXQQCLNZCBWNX3M5HHA4B7L2YDM2KYJ6SR4I4JBHSJU47ZINWGJM3";
const donor_06_s = "SAZKS6YT76X5I4IB4CC6GS2Q3VUISAALXODQ36HUC7E7XVHYFWJXVN37";
const donor_07 = "GBFJHOONGH22H5ZQ3FPB7NRNYFG3LIZW4PO2GWGOA4QC4OEG7DH7F3SL";
const donor_07_s = "SBFY4Y2JABXOJSJPHXWLOBDXBHAKE3WSPDRCKJAF57VJKE5IBERFR5ZJ";
const donor_08 = "GBOEDFIYMORSNE2MWH45H6LQNNCB3HWDRIUX4NDUJ4IJ3K5LWMN27XLZ";
const donor_08_s = "SAKSV6MCMVWR4SMM6PBFPDK3DPAOWFOOKR6ENBC5NF3BA6BGQHWR5IYB";
const donor_09 = "GDMMWANC5PFSLZLL2ZOAH47RSCSDA67NKPVTESOP3KAR5Y32DZGPPETU";
const donor_09_s = "SANSJGTCKZ3JRFCOXNDF5U7MEGVYAMF4INODX7ETASOHQ5G3GRJCNPK3";
const donor_10 = "GD3LXOYCJNNA7JBHR24S5ZUZJZOX7POHUQFMYPUQ3IPMT5VCIH3N7HOL";
const donor_10_s = "SA2W5UC4IT6EEFH73VOR6H7VTNILD2WKTRXAIMJGKDICRUFCN2LLZI3J";
const donor_11 = "GDD5YF4ASW4JJGXVS4GAX3OXYL5ALZ36B47GRARMFRB7OVSOBCALJYBZ";
const donor_11_s = "SCHNKYPGNEW7FCEFP5TXTI2LEM65QG4LGWUGWJTVWIEVI2EQ3GZML5WQ";

const donor_21 = "GAQ567LZPTHA3GESBK2NHOS36UCQRKN4GRT6UACISI7FQRHVV4PG2UJW";
const donor_21_s = "SBCC3SHXBGKWDEPJCBYNEVO2SZPVOEOXEUAS7PMD457H2QYGG4DXGTOZ";
const donor_22 = "GBXQOXZP44QCJ7TQ3576GIMD7BKZQUKDXOOZUQLZD2R5HJFZWB6A2KED";
const donor_22_s = "SC2Z32V73V66QAF73VE4LSKIDBKPHBDKJGD2C3VCRFIPCRHA2REWJ5CF";
// await Deposit(donor_22, donor_22_s, "xlm", 9900_0000000n);
// await ToggleCollateralSetting(donor_22, donor_22_s, "xlm");
// await Borrow(donor_06, donor_06_s, "eth", 300000_000000_000000n);
// await Borrow(donor_22, donor_22_s, "usdc", 500_000000n);
// await Borrow(donor_07, donor_07_s, "xlm", 8000_0000000n);


let tvl_decimal8 = await GetTVL();
let tvl = Number.parseFloat(100n * tvl_decimal8 / 100_000_000n) / 100;
console.log(`Total Value Locked: ${tvl} USD`)
// process.exit(0);

// await Borrow(admin, admin_secret, "xlm", 1_0000000n);
// await GetUserMaxAllowedBorrowAmountUsd(admin);
// await GetAvailableToBorrow(admin, "xlm");

console.log("========== Start ==========");

// await Redeem(admin, admin_secret, "usdc", 990000_000000n);
// await Redeem(admin, admin_secret, "eth", 990_000000_000000_000000n);

// const xlm_price = await GetPrice("xlm");
// const usdc_price = await GetPrice("usdc");
// const eth_price = await GetPrice("eth");

// const prices = await fetchCryptoPrices();
// console.log(`The current XLM price is ${prices.xlmPrice} USD.`);
// console.log(`The current ETH price is ${prices.ethPrice} USD.`);

// await UpdatePrice("xlm", BigInt(prices.xlmPrice * 100_000_000));
// await UpdatePrice("eth", BigInt(prices.ethPrice * 100_000_000));
// await UpdatePrice("usdc", 100_000_000n); // 1 USD

// console.log(" xlm price:", xlm_price, Number.parseFloat(10000n * xlm_price / 100_000_000n) / 10000);
// console.log(" usdc price:", usdc_price, Number.parseFloat(10000n * usdc_price / 100_000_000n) / 10000);
// console.log(" eth price:", eth_price, Number.parseFloat(10000n * eth_price / 100_000_000n) / 10000);

// process.exit(0);

// const xlm_liq = await GetLiquidityRate("xlm");
// const usdc_liq = await GetLiquidityRate("usdc");
// const eth_liq = await GetLiquidityRate("eth");
// console.log("Liq Rate XLM :", xlm_liq, Number.parseFloat(10000n * xlm_liq / 1_000000_000000_000000n) / 10000);
// console.log("Liq Rate USDC:", usdc_liq,  Number.parseFloat(10000n * usdc_liq / 1_000000_000000_000000n) / 10000);
// console.log("Liq Rate ETH :", eth_liq, Number.parseFloat(10000n * eth_liq / 1_000000_000000_000000n) / 10000);

// process.exit(0);

// let admin_atk = await token_balance(tokenA, admin);
// let admin_btk = await token_balance(tokenB, admin);
let admin_xlm = await token_balance(xlm_address, admin);
// let admin_deposit = await GetUserDepositedUsd(admin);
// let admin_atk_may_borrow = await GetAvailableToBorrow(admin, "atk");
console.log("Admin xlm balance:", admin_xlm, Number.parseFloat(10000n * admin_xlm / 10_000_000n) / 10000);
// console.log("      atk balance:", admin_atk, Number.parseFloat(10000n * admin_atk / 10_000_000n) / 10000);
// console.log("      btk balance:", admin_btk, Number.parseFloat(10000n * admin_btk / 10_000_000n) / 10000);
// console.log("      deposit usd:", admin_deposit, Number.parseFloat(10000n * admin_deposit / 100_000_000n) / 10000);
// console.log("       borrow atk:", admin_atk_may_borrow, Number.parseFloat(10000n * admin_atk_may_borrow / 10_000_000n) / 10000);

// let borrow_apy_atk = await GetInterestRate("atk");
// console.log("Borrow APY ATK:", Number.parseFloat( 1000n * borrow_apy_atk / 1_000000_000000_000000n) / 1000);
// let borrow_apy_xlm = await GetInterestRate("xlm");
// console.log("Borrow APY XLM:", Number.parseFloat( 1000n * borrow_apy_xlm / 1_000000_000000_000000n) / 1000);

// await Faucet(admin, admin_secret, USDC, 100_000_000000n);
// await Faucet(admin, admin_secret, ETH, 1000_000000_000000_000000n);

// process.exit(0);

// await UpdatePrice("xlm", 12_760_000n); // 0.1276 USD
// await UpdatePrice("usdc", 100_000_000n); // 1 USD
// await UpdatePrice("eth", 1917_41_000_000n); // 1917.41 USD
// await UpdatePrice("atk", 100_000_000n); // 1 USD
// await UpdatePrice("btk", 200_000_000n); // 2 USD

// await AddMarkets("xlm", xlm_address, 7);
// await Deposit(admin, admin_secret, "xlm", 100_0000000n);
// await AddMarkets("usdc", USDC, 6);
// await Deposit(admin, admin_secret, "usdc", 20_000_000000n);
// await AddMarkets("eth", ETH, 18);
// await Deposit(admin, admin_secret, "eth", 10_000000_000000_000000n);
// await AddMarkets("atk", tokenA, 7);
// await Deposit(admin, admin_secret, "atk", 1_000_0000000n);
// await AddMarkets("btk", tokenB, 7);
// await Deposit(admin, admin_secret, "btk", 1_000_0000000n);

// await SetTokenInterestRateModelParams(
//     "xlm",
//     5_000000_000000_000000n,
//     40_000000_000000_000000n,
//     70_000000_000000_000000n,
//     80_00000,
// );
// await SetTokenInterestRateModelParams(
//     "usdc",
//     5_000000_000000_000000n,
//     20_000000_000000_000000n,
//     100_000000_000000_000000n,
//     80_00000,
// );
// await SetTokenInterestRateModelParams(
//     "eth",
//     5_000000_000000_000000n,
//     50_000000_000000_000000n,
//     60_000000_000000_000000n,
//     80_00000,
// );
// process.exit(0);

// await Deposit(user1, user1_secret, "xlm", 100_0000000n);
// await ToggleCollateralSetting(user1, user1_secret, "xlm");
await Borrow(user1, user1_secret, "usdc", 1n * 1_000_000n); // 1 usdc
// await Borrow(user1, user1_secret, "btk", 1n * 10_000_000n); // 1 btk = 2 usd
// await Redeem(user1, user1_secret, "xlm", 0);
// await Repay(user1, user1_secret, "usdc", 0); // All usdc

let user1_xlm = await token_balance(xlm_address, user1);
// let user1_atk = await token_balance(tokenA, user1);
// let user1_btk = await token_balance(tokenB, user1);
let user1_deposit = await GetUserDepositedUsd(user1);
let user1_deposit_xlm = await GetDeposit(user1, "xlm");
// let user1_deposit_atk = await GetDeposit(user1, "atk");
let user1_borrowed = await GetUserBorrowedUsd(user1);
let user1_usdc_may_borrow = await GetAvailableToBorrow(user1, "usdc");
let user1_xlm_redeem = await GetAvailableToRedeem(user1, "xlm");
console.log("User1 xlm balance:", user1_xlm, Number.parseFloat(10000n * user1_xlm / 10_000_000n) / 10000);
// console.log("      atk balance:", user1_atk, Number.parseFloat(10000n * user1_atk / 10_000_000n) / 10000);
// console.log("      btk balance:", user1_btk, Number.parseFloat(10000n * user1_btk / 10_000_000n) / 10000);
console.log("      deposit usd :", user1_deposit, Number.parseFloat(10000n * user1_deposit / 100_000_000n) / 10000);
console.log("      deposit xlm :", user1_deposit_xlm, Number.parseFloat(10000n * user1_deposit_xlm / 10_000_000n) / 10000);
// console.log("      deposit atk :", user1_deposit_atk, Number.parseFloat(10000n * user1_deposit_atk / 10_000_000n) / 10000);
console.log("      borrowed usd:", user1_borrowed, Number.parseFloat(10000n * user1_borrowed / 100_000_000n) / 10000);
console.log("       may borrow usdc:", user1_usdc_may_borrow, Number.parseFloat(10000n * user1_usdc_may_borrow / 1_000_000n) / 10000);
console.log("       redeem xlm:", user1_xlm_redeem, Number.parseFloat(10000n * user1_xlm_redeem / 10_000_000n) / 10000);

process.exit(0);

let user2_xlm = await token_balance(xlm_address, user2);
let user2_atk = await token_balance(tokenA, user2);
let user2_deposit = await GetUserDepositedUsd(user2);
let user2_deposit_xlm = await GetDeposit(user2, "xlm");
let user2_borrowed = await GetUserBorrowedUsd(user2);
let user2_usdc_may_borrow = await GetAvailableToBorrow(user2, "usdc");
let user2_xlm_redeem = await GetAvailableToRedeem(user2, "xlm");
// let user2_atk_redeem = await GetAvailableToRedeem(user2, "atk");
console.log("User2 xlm balance:", user2_xlm, Number.parseFloat(10000n * user2_xlm / 10_000_000n) / 10000);
console.log("      atk balance:", user2_atk, Number.parseFloat(10000n * user2_atk / 10_000_000n) / 10000);
console.log("      deposit usd:", user2_deposit, Number.parseFloat(10000n * user2_deposit / 100_000_000n) / 10000);
console.log("      deposit xlm :", user2_deposit_xlm, Number.parseFloat(10000n * user2_deposit_xlm / 10_000_000n) / 10000);
console.log("      borrowed usd:", user2_borrowed, Number.parseFloat(10000n * user2_borrowed / 100_000_000n) / 10000);
console.log("       may borrow usdc:", user2_usdc_may_borrow, Number.parseFloat(10000n * user2_usdc_may_borrow / 10_000_000n) / 10000);
console.log("       redeem xlm:", user2_xlm_redeem, Number.parseFloat(10000n * user2_xlm_redeem / 10_000_000n) / 10000);
// console.log("       redeem atk:", user2_atk_redeem, Number.parseFloat(10000n * user2_atk_redeem / 10_000_000n) / 10000);

process.exit(0);
