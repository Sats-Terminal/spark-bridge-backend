import { Edict, none, RuneId, Runestone, some } from 'runelib';
import { getTransaction, sendRawTransaction } from './bitcoin-client';
import { Payment, Psbt } from 'bitcoinjs-lib';
import * as bitcoin from 'bitcoinjs-lib';
import { ECPairInterface } from 'ecpair';
import { toXOnly } from 'bitcoinjs-lib/src/psbt/bip371';
import { toBitcoinSigner } from './utils';

const network = bitcoin.networks.regtest;

export interface MintRuneParams {
	keyPair: ECPairInterface;
	utxo: {
		txid: string;
		vout: number;
		value: number;
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

	const { output } = bitcoin.payments.p2tr({
		internalPubkey: toXOnly(Buffer.from(keyPair.publicKey)),
		network,
	});

	psbt.addInput({
		hash: utxo.txid,
		index: utxo.vout,
		witnessUtxo: { value: utxo.value, script: output! },
		tapInternalKey: toXOnly(Buffer.from(keyPair.publicKey)),
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

	let tweakedKeyPair = keyPair.tweak(
		bitcoin.crypto.taggedHash('TapTweak', toXOnly(Buffer.from(keyPair.publicKey)))
	)
	let signer = toBitcoinSigner(tweakedKeyPair);

	psbt.signInput(0, signer);

	psbt.finalizeAllInputs();
	const tx = psbt.extractTransaction();
	const raw = tx.toHex();
	const txid = await sendRawTransaction(raw);

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