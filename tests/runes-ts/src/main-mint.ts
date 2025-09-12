import { ECPairFactory } from "ecpair";
import * as bitcoin from 'bitcoinjs-lib';
import * as tinySecp256k1 from 'tiny-secp256k1';
import { getRuneId, getTransaction, initDefaultWallet } from "./bitcoin-client";
import { mintRune } from "./mint";
import { generateBlocks } from "./bitcoin-client";
import { toXOnly } from "./utils";

bitcoin.initEccLib(tinySecp256k1);
const ECPair = ECPairFactory(tinySecp256k1);
const network = bitcoin.networks.regtest;

const WIF_PRIVATE_KEY = 'cSYFixQzjSrZ4b4LBT16Q7RXBk52DZ5cpJydE7DzuZS1RhzaXpEN';
const etchTxId = '572dfcf4da27fccd41a868cc88f48126a569af44c838b4df9bcc00f9d9aa138d';
const changeVout = 2;
const etchingVout = 1;

async function main() {
  console.log('---- Minting phase ----');

	const keyPair = ECPair.fromWIF(WIF_PRIVATE_KEY, network);


	console.log('\n1. Initializing wallet and getting funds...');

	await initDefaultWallet();

	console.log('✅ Wallet initialized and funded');


	console.log('\n2. Getting rune ID...');

	const runeId = await getRuneId(etchTxId);

	console.log(`Rune ID: ${runeId.block}:${runeId.idx}`);
	console.log('✅ Rune ID obtained');

	console.log('\n get output address...');

	const outputAddress = bitcoin.payments.p2tr({
		internalPubkey: toXOnly(Buffer.from(keyPair.publicKey)),
		network,
	});

	console.log('Output address:', outputAddress.address);
	console.log('✅ Output address obtained');


	console.log('\n3. Minting rune...');

	const mintRuneResponse = await mintRune({
		keyPair: ECPair.fromWIF(WIF_PRIVATE_KEY, network),
		etchUtxo: { txid: etchTxId, vout: etchingVout, value: 100000000 },
		fundedUtxo: { txid: etchTxId, vout: changeVout, value: 100000000 },
		mintAmount: 100000000,
		runeId: runeId,
		outputAddress: outputAddress.address!,
	});

	await generateBlocks(6);
	await new Promise(resolve => setTimeout(resolve, 2000));

	console.log('✅ Rune minted successfully!');


	console.log('\n4. Getting rune information from Titan...');

	const transaction = await getTransaction(mintRuneResponse.mintingUtxo.txid);
	
	let rune = transaction.output[0].runes[0];
	console.log('Rune:', rune);

	if (!rune || rune.amount === '0') {
		throw new Error('Rune not found in transaction output');
	}

	console.log('✅ Rune found in transaction output');

	console.log('\n✅ Rune minted successfully!');
}


main();