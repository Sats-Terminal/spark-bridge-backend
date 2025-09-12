import * as bitcoin from 'bitcoinjs-lib';
import { sendRawTransaction } from './bitcoin-client';
import { ECPairInterface } from 'ecpair';


export function toXOnly(pubkey: Buffer): Buffer {
	return pubkey.subarray(1, 33);
}

export async function signAndSend(keyPair: ECPairInterface, psbt: bitcoin.Psbt, taprootIndexes: number[] = []) {
	for (let i = 0; i < psbt.inputCount; i++) {
		const isTaproot = taprootIndexes.includes(i);
		console.log('Signing input', i, 'isTaproot:', isTaproot);

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

	psbt.finalizeAllInputs();
	const tx = psbt.extractTransaction();
	const raw = tx.toHex();
	
	const txid = await sendRawTransaction(raw);
	console.log('Broadcasted transaction:', txid);
	return txid;
}
