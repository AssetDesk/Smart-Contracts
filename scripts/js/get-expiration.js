import { xdr, Server, Contract, Address, hash, scValToNative } from 'soroban-client';

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

// Configure SorobanClient to talk to the soroban-rpc
const server = new Server(
    rpc_url, { allowHttp: true }
  );

async function sorobanCliBump(xdrKey, ledgersToBump) {
  const { stdout, stderr } = await exec(`soroban contract bump --id ${contract_address} --key-xdr ${xdrKey} --source ${admin_secret} --rpc-url https://rpc-futurenet.stellar.org:443/ --network-passphrase "Test SDF Future Network ; October 2022" --durability persistent --ledgers-to-expire ${ledgersToBump}`);
  console.log('  stdout:', stdout);
  if (stderr != "") {console.log('  stderr:', stderr);}
}

const getKey_String = (key_string) => {
    let key = xdr.ScVal.scvVec([
        xdr.ScVal.scvSymbol(key_string),
    ]);
    return key;
}

const getKey_TOTAL_BORROW_DATA = (denom) => {
    let key = xdr.ScVal.scvVec([
        xdr.ScVal.scvSymbol("TOTAL_BORROW_DATA"), 
        xdr.ScVal.scvSymbol(denom),
    ]);
    return key;
}

const getExpirationKey = (ledger_key) => {
    let contractKey = xdr.LedgerKey.contractData(
        new xdr.LedgerKeyContractData({
            contract: new Contract(contract_address).address().toScAddress(),
            key: ledger_key,
            durability: xdr.ContractDataDurability.persistent()
        })
    ).toXDR();

    let keyHash = hash(contractKey);
    const expirationKey = xdr.LedgerKey.expiration(
        new xdr.LedgerKeyExpiration({ keyHash }),
    ).toXDR("base64");

    return expirationKey;
}


const getContractExpirationKey = (contract_address) => {
    let contractKey = xdr.LedgerKey.contractData(
        new xdr.LedgerKeyContractData({
            contract: new Contract(contract_address).address().toScAddress(),
            key: new xdr.ScVal.scvLedgerKeyContractInstance(),
            durability: xdr.ContractDataDurability.persistent(),
        })
    ).toXDR();

    let keyHash = hash(contractKey);
    const expirationKey = xdr.LedgerKey.expiration(
        new xdr.LedgerKeyExpiration({ keyHash }),
    ).toXDR("base64");

    return expirationKey;
}

async function getInstanceValue(contract_address) {
    const instanceKey = xdr.LedgerKey.contractData(
        new xdr.LedgerKeyContractData({
            contract: new Address(contract_address).toScAddress(),
            key: xdr.ScVal.scvLedgerKeyContractInstance(),
            durability: xdr.ContractDataDurability.persistent(),
        })
    );

    const response = await server.getLedgerEntries([instanceKey,instanceKey]);
    const dataEntry = xdr.LedgerEntryData.fromXDR(response.entries[0].xdr, 'base64');
    return dataEntry.contractData().val().instance();
}

async function getContractHashExpirationKey (contract_address) {
    
    let instance = await getInstanceValue(contract_address)
    let wasmHash = instance.executable().wasmHash();

    const contractCodeXDR = xdr.LedgerKey.contractCode(
        new xdr.LedgerKeyContractCode({
        hash: Buffer.from(wasmHash, 'hex'),
        })
    );
    let keyHash = hash(contractCodeXDR.toXDR());
    // console.log(keyHash.toString('hex'));
    const expirationKey = xdr.LedgerKey.expiration(
        new xdr.LedgerKeyExpiration({ keyHash }),
    ).toXDR("base64");

    return expirationKey;
}


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
        const entry_data = xdr.LedgerEntryData.fromXDR(response_json.result.entries[i].xdr, 'base64');
        const keyHash = xdr.LedgerKey.expiration(new xdr.LedgerKeyExpiration({keyHash: entry_data.expiration().keyHash()})).toXDR("base64");
        const final_ledger = entry_data.expiration().expirationLedgerSeq()
        expiration_json[keyHash] = final_ledger;
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

const namedKeys = {
    "Admin            ": getExpirationKey(getKey_String("Admin")),
    "Liquidator       ": getExpirationKey(getKey_String("Liquidator")),
    "Contract Lending": getContractExpirationKey(contract_address),
    // "Contract Hash": await getContractHashExpirationKey(contract_address),
    "Contract Token A": getContractExpirationKey(tokenA),
    "Contract Token B": getContractExpirationKey(tokenB),
    // "Token Contract Hash": await getContractHashExpirationKey(tokenA)
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