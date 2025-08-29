import { Psbt } from 'bitcoinjs-lib';
import { call_rpc } from '../titan_test/rpc_utils';

export async function signAndSend(
  keyPair: any,
  psbt: Psbt,
  taprootIndexes: number[],
) {
  // if (process.env.NODE) {

  for (let i = 0; i < psbt.inputCount; i++) {
    let isTaproot = taprootIndexes.findIndex((v) => v == i) != -1;
    console.log(psbt.txInputs[i], isTaproot);

    psbt.signInput(i, {
      publicKey: Buffer.from(keyPair.publicKey),
      sign: (hash) => {
        const signature = keyPair.sign(hash);
        return Buffer.from(signature);
      },
      signSchnorr: (hash) => {
        const signature = keyPair.signSchnorr(hash);
        return Buffer.from(signature);
      },
    });
  }

  // Only use the Taproot-compatible signer for Taproot inputs
  // psbt.signInput(0, keyPair);
  // You’re spending via tapscript path (because of tapLeafScript), finalize normally:
  psbt.finalizeAllInputs();
  const tx = psbt.extractTransaction();
  const raw = tx.toHex();
  const txid = await call_rpc('sendrawtransaction', [raw]);
  console.log('Broadcasted:', txid);
}

export function toXOnly(pubkey: Buffer): Buffer {
  return pubkey.subarray(1, 33);
}
