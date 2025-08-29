import { Psbt, networks, payments, script, Transaction } from 'bitcoinjs-lib';
import { ECPairFactory } from 'ecpair';
import * as ecc from '@bitcoinerlab/secp256k1';
import { toXOnly } from 'bitcoinjs-lib/src/psbt/bip371';

import {
  Runestone,
  Rune,
  Etching,
  Terms,
  Range,
  some,
  none,
  EtchInscription,
  RuneId,
} from 'runelib'; // <- adjust import
import { toOutputScript } from 'bitcoinjs-lib/src/address';
import logger from '../src/logger';

const ECPair = ECPairFactory(ecc);

// ----------- CONFIG: fill these -----------
const network = networks.regtest;
const RPC_USER = 'bitcoin';
const RPC_PASS = 'bitcoinpass';
const RPC_URL = 'http://127.0.0.1:18443/';
// Wallet Import Format (WIF)
const YOUR_WIF = 'cSYFixQzjSrZ4b4LBT16Q7RXBk52DZ5cpJydE7DzuZS1RhzaXpEN';

const txid = '6e14b295261c93ca9da4f3683675443e7569c2e0ac00422b6efbf66797859e6c';
const vout = 0;
const value = 5000000000;
const txHex =
  '020000000001010000000000000000000000000000000000000000000000000000000000000000ffffffff025100ffffffff0200f2052a01000000160014e2b97f3eaaa8ed2a0ab03033b61627b3f0c9292e0000000000000000266a24aa21a9ede2f61c3f71d1defd3fa999dfa36953755c690689799962b48bebd836974e8cf90120000000000000000000000000000000000000000000000000000000000000000000000000';
// -----------------------------------------

function call_rpc(method: string, params: any[] = []): any {
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

// https://developer.bitcoin.org/reference/rpc/getnewaddress.html
async function mine(n = 1, address?: string) {
  const addr = address ?? (await call_rpc('b', ['', 'bech32m'])); // regtest bech32m is fine
  return call_rpc('generatetoaddress', [n, addr]);
}

async function signAndSend(keyPair: any, psbt: Psbt) {
  // Sighash default for Taproot
  for (let i = 0; i < psbt.inputCount; i++) {
    logger.info(psbt.txInputs[i]);

    psbt.signInput(i, {
      publicKey: Buffer.from(keyPair.publicKey),
      sign: (hash) => {
        const signature = keyPair.sign(hash);
        return Buffer.from(signature);
      },
    });
  }
  // You’re spending via tapscript path (because of tapLeafScript), finalize normally:
  psbt.finalizeAllInputs();
  const tx = psbt.extractTransaction();
  const raw = tx.toHex();
  const txid = await call_rpc('sendrawtransaction', [raw]);
  // Mine a block so it confirms on regtest
  // await mine(1);
  console.log('Broadcasted:', txid);
  return txid;
}

async function mint() {
  const keyPair = ECPair.fromWIF(YOUR_WIF, network);

  const mintstone = new Runestone([], none(), some(new RuneId(1, 0)), some(1));

  const { address } = payments.p2wpkh({
    pubkey: Buffer.from(keyPair.publicKey),
    network,
  });
  console.log('address:', address.toString());

  logger.info('address:', address.toString());

  logger.info(`Using UTXO ${txid}:${vout}`);

  const psbt = new Psbt({ network });
  psbt.addInput({
    hash: txid,
    index: vout,
    nonWitnessUtxo: Buffer.from(txHex, 'hex'),
  });

  psbt.addOutput({
    script: mintstone.encipher(),
    value: 0,
  });

  psbt.addOutput({
    address: address!, // ord address
    value: 546,
  });

  const fee = 5000;

  const change = value - fee - 546;

  psbt.addOutput({
    address: address!, // change address
    value: change,
  });

  await signAndSend(keyPair, psbt);
}

mint().catch((e) => {
  console.error(e);
  process.exit(1);
});
