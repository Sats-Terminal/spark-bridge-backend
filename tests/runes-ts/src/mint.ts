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
	};
	fundedUtxo: {
		txid: string;
		vout: number;
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

	// Create P2WPKH script for witnessUtxo
	const p2wpkhScript = bitcoin.payments.p2wpkh({
		pubkey: Buffer.from(keyPair.publicKey),
		network,
	}).output!;

	const etchTransaction = await getTransaction(etchUtxo.txid);
	const etchValue = etchTransaction.output[etchUtxo.vout].value;

	psbt.addInput({
		hash: etchUtxo.txid,
		index: etchUtxo.vout,
		witnessUtxo: { value: etchValue, script: p2wpkhScript },
	});

	const fundedTransaction = await getTransaction(fundedUtxo.txid);
	const fundedValue = fundedTransaction.output[fundedUtxo.vout].value;

	psbt.addInput({
		hash: fundedUtxo.txid,
		index: fundedUtxo.vout,
		witnessUtxo: { value: fundedValue, script: p2wpkhScript },
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
	const change = fundedValue + etchValue - dustLimit - fee;

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