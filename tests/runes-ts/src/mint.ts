import { Edict, none, RuneId, Runestone, some } from 'runelib';
import { getTransaction } from './bitcoin-client';
import { Psbt } from 'bitcoinjs-lib';
import * as bitcoin from 'bitcoinjs-lib';
import { signAndSend } from './utils';
import { ECPairInterface } from 'ecpair';

const network = bitcoin.networks.regtest;

export interface MintRuneParams {
	keyPair: ECPairInterface;
	etchUtxo: {
		txid: string;
		vout: number;
		value: number;
	};
	fundedUtxo: {
		txid: string;
		vout: number;
		value: number;
	};
	outputAddress: string;
	mintAmount: number;
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
  const { etchUtxo, fundedUtxo, mintAmount, runeId, outputAddress, keyPair } = params;
  
	const edict = new Edict(runeId, BigInt(mintAmount), 1);

	const mintstone = new Runestone([edict], none(), some(runeId), some(1));

	const psbt = new Psbt({ network });

	const etchTransaction = await getTransaction(etchUtxo.txid);

	let etchScript = etchTransaction.output[etchUtxo.vout].script_pubkey;
	console.debug('etchScript:', etchScript);

	psbt.addInput({
		hash: etchUtxo.txid,
		index: etchUtxo.vout,
		witnessUtxo: { value: etchUtxo.value, script: Buffer.from(etchScript) },
	});

	const fundedTransaction = await getTransaction(fundedUtxo.txid);

	let fundedScript = fundedTransaction.output[fundedUtxo.vout].script_pubkey;
	console.debug('fundedScript:', fundedScript);

	psbt.addInput({
		hash: fundedUtxo.txid,
		index: fundedUtxo.vout,
		witnessUtxo: { value: mintAmount, script: Buffer.from(fundedScript) },
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
	const change = mintAmount - dustLimit - fee;

	psbt.addOutput({
		address: outputAddress,
		value: change,
	});

	const txid = await signAndSend(keyPair, psbt, [0]);

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