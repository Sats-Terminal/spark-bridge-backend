import { RuneResponse, TitanHttpClient, AddressData, Transaction } from "@titanbtcio/sdk";
import * as runelib from 'runelib';

const titanClient = new TitanHttpClient('http://localhost:3030');

const RPC_USER = 'bitcoin';
const RPC_PASS = 'bitcoinpass';
const RPC_URL = 'http://127.0.0.1:18443/';

async function callRpc(method: string, params: any[] = []): Promise<any> {
  return fetch(RPC_URL, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Authorization:
        'Basic ' + Buffer.from(`${RPC_USER}:${RPC_PASS}`).toString('base64'),
    },
    body: JSON.stringify({ jsonrpc: '2.0', id: '0', method, params }),
  }).then(async (r) => {
    const data: any = await r.json();
    if (data.error) throw new Error(JSON.stringify(data.error));
    return data.result;
  });
}

async function generateToAddress(address: string, n: number) {

  return callRpc('generatetoaddress', [n, address]);
}

async function createDefaultWallet() {
  let wallets = await listWallets();
  if (!wallets.includes("testwallet")) {
    await callRpc('createwallet', ['testwallet']);
  }
}

export async function getDefaultWalletBalance() {
  return callRpc('getbalance', ['*', 6]);
}

async function listWallets(): Promise<string[]> {
  return callRpc('listwallets');
}

async function getNewAddress(type?: string): Promise<string> {
  return callRpc('getnewaddress', ['', type ?? 'bech32m']);
}

// return txid
export async function faucet(address: string, btcAmount: number): Promise<string> {
  return await callRpc('sendtoaddress', [address, btcAmount]);
}

export async function sendRawTransaction(txHex: string): Promise<string> {
  return callRpc('sendrawtransaction', [txHex]);
}

export async function generateBlocks(n: number): Promise<string[]> {
  const address = await getNewAddress();
  return await generateToAddress(address, n);
}

export async function initDefaultWallet() {
  await createDefaultWallet();
  let address = await getNewAddress();
  await generateToAddress(address, 100);
  await generateBlocks(100);
}

export async function getRuneId(txid: string): Promise<runelib.RuneId> {
  const transaction = await titanClient.getTransaction(txid);
  if (!transaction) {
    throw new Error(`Transaction ${txid} not found in Titan`);
  }
  const blockHeight = transaction.status.block_height;
  if (!blockHeight) {
    throw new Error(`Block height not found for transaction ${txid}`);
  }
  const block = await titanClient.getBlock(blockHeight.toString());
  if (!block) {
    throw new Error(`Block ${blockHeight} not found in Titan`);
  }
  const txIndex = block.tx_ids.findIndex(id => id === txid);
  if (txIndex === -1) {
    throw new Error(`Transaction ${txid} not found in block ${blockHeight}`);
  }
  console.log('blockHeight:', blockHeight);
  console.log('txIndex:', txIndex);
  return new runelib.RuneId(blockHeight, txIndex);
}

export async function getRune(runeId: runelib.RuneId): Promise<RuneResponse> {
  const runeString = `${runeId.block}:${runeId.idx}`
  const someRune = await titanClient.getRune(runeString);

  if (!someRune) {
    throw new Error(`Rune ${runeId} not found in Titan`);
  }
  return someRune
}

export async function getAddressData(address: string): Promise<AddressData> {
  return await titanClient.getAddress(address);
}

export async function getTransaction(txid: string): Promise<Transaction> {
  let transaction = await titanClient.getTransaction(txid);
  if (!transaction) {
    throw new Error(`Transaction ${txid} not found in Titan`);
  }
  return transaction;
}