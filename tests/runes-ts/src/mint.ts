import { Edict, none, RuneId, Runestone, some } from 'runelib';
import { getTransaction } from './bitcoin-client';
import { Payment, Psbt } from 'bitcoinjs-lib';
import * as bitcoin from 'bitcoinjs-lib';
import { signAndSend } from './utils';
import { ECPairInterface } from 'ecpair';

const network = bitcoin.networks.regtest;

export interface MintRuneParams {
	keyPair: ECPairInterface;
	utxo: {
		txid: string;
		vout: number;
		value: number;
		p2trInput: Payment;
	};
	outputAddress: string;
	runeId: RuneId;
}

export interface MintRuneResponse {
	changeUtxo: {
		txid: string;
		vout: number;
		value: number;
	};
	mintingUtxo: {
		txid: string;
		vout: number;
		value: number;
	};
}

export async function mintRune(params: MintRuneParams) {
  const { utxo, runeId, outputAddress, keyPair } = params;
  
	const mintstone = new Runestone([], none(), some(runeId), some(1));

	const psbt = new Psbt({ network });

	psbt.addInput({
		hash: utxo.txid,
		index: utxo.vout,
		witnessUtxo: { value: utxo.value, script: utxo.p2trInput.output! },
	});

	psbt.addOutput({
		script: mintstone.encipher(),
		value: 0,
	});

	const dustLimit = 546;

	psbt.addOutput({
		address: outputAddress,
		value: dustLimit,
	});

	const fee = 5000;
	const change = utxo.value - dustLimit - fee;

	psbt.addOutput({
		address: outputAddress,
		value: change,
	});

	const txid = await signAndSend(keyPair, psbt, []);

	await new Promise(resolve => setTimeout(resolve, 2000));

	return {
		changeUtxo: {
			txid: txid,
			vout: 2,
			value: change,
		},
		mintingUtxo: {
			txid: txid,
			vout: 1,
			value: dustLimit,
		},
	};
}	