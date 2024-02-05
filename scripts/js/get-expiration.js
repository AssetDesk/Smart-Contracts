// import { xdr, Server, Contract, Address, hash, scValToNative } from 'soroban-client';
import {
    scValToNative,
    nativeToScVal,
    SorobanRpc,
    Contract,
    xdr,
  } from 'stellar-sdk';

import util from 'util'
import child_process from 'child_process'

const exec = util.promisify(child_process.exec);
const DAY_IN_LEDGERS = 17280;
const WEEK_BUMP_AMOUNT = 7 * DAY_IN_LEDGERS;
const MONTH_BUMP_AMOUNT = 30 * DAY_IN_LEDGERS;

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
const USDC = process.env.USDC;
const ETH = process.env.ETH;
const FAUCET = process.env.FAUCET;

// Configure SorobanClient to talk to the soroban-rpc
const server = new SorobanRpc.Server(
    rpc_url, { allowHttp: true }
  );

  async function sorobanCliBump(xdrKey, ledgersToBump) {
    const { stdout, stderr } = await exec(`soroban contract extend --id ${settings.contract.address} --key-xdr ${xdrKey} --source ${settings.admin.secret} --rpc-url https://rpc-futurenet.stellar.org:443/ --network-passphrase "Test SDF Future Network ; October 2022" --durability persistent --ledgers-to-extend ${ledgersToBump}`);
    console.log('  stdout:', stdout);
    if (stderr != "") {console.log('  stderr:', stderr);}
  }
  
  async function sorobanCliRestore(xdrKey) {
      const { stdout, stderr } = await exec(`soroban contract restore --id ${settings.contract.address} --key-xdr ${xdrKey} --source ${settings.admin.secret} --rpc-url https://rpc-futurenet.stellar.org:443/ --network-passphrase "Test SDF Future Network ; October 2022" --durability persistent`);
      console.log('  stdout:', stdout);
      if (stderr != "") {console.log('  stderr:', stderr);}
    }

const getKey_String = (key_string) => {
    let key = xdr.ScVal.scvVec([
        xdr.ScVal.scvSymbol(key_string),
    ]);
    return key;
}

const getKeyNameAddress = (name, address) => {
    let key = xdr.ScVal.scvVec([
        xdr.ScVal.scvSymbol(name), 
        nativeToScVal(address, {type: "address"}),
    ]);
    return key;
}


const getContractExpirationKey = (contract_address) => {
    let contractKey = xdr.LedgerKey.contractData(
        new xdr.LedgerKeyContractData({
            contract: new Contract(contract_address).address().toScAddress(),
            key: new xdr.ScVal.scvLedgerKeyContractInstance(),
            durability: xdr.ContractDataDurability.persistent(),
        })
    ).toXDR("base64");

    // let keyHash = hash(contractKey);
    // const expirationKey = xdr.LedgerKey.expiration(
    //     new xdr.LedgerKeyExpiration({ keyHash }),
    // ).toXDR("base64");

    return contractKey;
}

async function getContractHashExpirationKey (contract_address) {
    
    const server = new SorobanRpc.Server(rpc_url);
    const contractKey = await getLedgerKeyContractCode(contract_address);
    const response = await server.getLedgerEntries(contractKey);
    const entry = response.entries[0].val;
    const instance = entry.contractData().val().instance();
    let wasm_hash_key = xdr.LedgerKey.contractCode(
        new xdr.LedgerKeyContractCode({
        hash: instance.executable()._value
        })
    );
    // const wasm_code = await server.getLedgerEntries(wasm_hash_key);
    // console.log(wasm_code); 

    return wasm_hash_key.toXDR("base64");
}

function getLedgerKeyContractCode(contractId) {
    const contract  = new Contract(contractId);
    // console.log(contract.getFootprint());
    const instance = contract.getFootprint();
    return instance;
  }


// Make a batch POST request

// Make a batch POST request
async function makeBatchRequest(keys) {

    const jsonData = {
        jsonrpc: '2.0',
        id: 1,
        method: 'getLedgerEntries',
        params: {
          keys: keys
        }
    }; 
    const requestOptions = {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(jsonData)
    };

    const response = await fetch(rpc_url, requestOptions);
    const response_json = await response.json();
    // console.log(response_json);
    let expiration_json = {}
    for (const i in response_json.result.entries){
        // console.log(response_json.result.entries[i]);
        // const entry_data = xdr.LedgerEntryData.fromXDR(response_json.result.entries[i].xdr, 'base64');
        // const keyHash = xdr.LedgerKey.expiration(new xdr.LedgerKeyExpiration({keyHash: entry_data.expiration().keyHash()})).toXDR("base64");
        // const final_ledger = entry_data.expiration().expirationLedgerSeq()
        const keyHash = response_json.result.entries[i].key;
        expiration_json[keyHash] = response_json.result.entries[i].liveUntilLedgerSeq;
    }

    expiration_json["latestLedger"] = response_json.result.latestLedger;

    return expiration_json;
}

function secondsToMonthsWeeksDaysHoursString(seconds) {
    const months = Math.floor(seconds / (60 * 60 * 24 * 30));
    const weeks = Math.floor((seconds % (60 * 60 * 24 * 30)) / (60 * 60 * 24 * 7));
    const days = Math.floor((seconds % (60 * 60 * 24 * 7)) / (60 * 60 * 24));
    const hours = Math.floor((seconds % (60 * 60 * 24)) / (60 * 60));
  
    const timeString = `${months ? months + ' months' : ''}${weeks ? ' ' + weeks + ' weeks' : ''}${days ? ' ' + days + ' days' : ''}${hours ? ' ' + hours + ' hours' : ''}`;
    return timeString.trim();
}

const getLedgerKeySymbol = (contract, key) => {
  
    let contractKey = xdr.LedgerKey.contractData(
      new xdr.LedgerKeyContractData({
        contract: new Contract(contract).address().toScAddress(),
        key,
        durability: xdr.ContractDataDurability.persistent()
      })
    ).toXDR("base64");
    return contractKey;
  } 

const namedKeys = {
    "Admin            ": getLedgerKeySymbol(contract_address, getKey_String("Admin")),
    "Liquidator       ": getLedgerKeySymbol(contract_address, getKey_String("Liquidator")),
    "TotalBorrowData  ": getLedgerKeySymbol(contract_address, getKey_String("TotalBorrowData")),
    "Prices           ": getLedgerKeySymbol(contract_address, getKey_String("Prices")),
    "SupportedTokensInfo ": getLedgerKeySymbol(contract_address, getKey_String("SupportedTokensInfo")),
    "SupportedTokensList ": getLedgerKeySymbol(contract_address, getKey_String("SupportedTokensList")),
    "LiquidityIndexData  ": getLedgerKeySymbol(contract_address, getKey_String("LiquidityIndexData")),
    "ReserveConfiguration          ": getLedgerKeySymbol(contract_address, getKey_String("ReserveConfiguration")),
    "TokensInterestRateModelParams ": getLedgerKeySymbol(contract_address, getKey_String("TokensInterestRateModelParams")),
    "UserMMTokenBalance      user1 ": getLedgerKeySymbol(contract_address, getKeyNameAddress("UserMMTokenBalance", user1)),
    "UserDepositAsCollateral user1 ": getLedgerKeySymbol(contract_address, getKeyNameAddress("UserDepositAsCollateral", user1)),
    "UserBorrowingInfo       user1 ": getLedgerKeySymbol(contract_address, getKeyNameAddress("UserBorrowingInfo", user1)),
    "Contract Lending": getContractExpirationKey(contract_address),
    "Contract USDC   ": getContractExpirationKey(USDC),
    "Contract ETH    ": getContractExpirationKey(ETH),
    "Contract Faucet ": getContractExpirationKey(FAUCET),
    "WASM Contract": await getContractHashExpirationKey(contract_address),
    "WASM Tokens  "  : await getContractHashExpirationKey(USDC),
    "WASM Faucet  "  : await getContractHashExpirationKey(FAUCET),
}


let expire_data = await makeBatchRequest(Object.values(namedKeys));
const latest_ledger = Number(expire_data.latestLedger);
delete expire_data.latestLedger;

console.log("Latest ledger:", latest_ledger);
for (const key in expire_data) {
    if (expire_data[key] > latest_ledger) {
        expire_data[key] = secondsToMonthsWeeksDaysHoursString((expire_data[key] - latest_ledger) * 5);
    } else {
        expire_data[key] = "Expired"
    }
}
let expiration_time = {}
for (const name in namedKeys) {
    expiration_time[name] = expire_data[namedKeys[name]];
}
console.log(expiration_time);

// console.log("  Bumping Admin...");
// await sorobanCliBump(getKey_String("Admin").toXDR("base64"), MONTH_BUMP_AMOUNT);
