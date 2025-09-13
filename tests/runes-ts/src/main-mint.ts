import { ECPairFactory } from "ecpair";
import * as bitcoin from 'bitcoinjs-lib';
import * as tinySecp256k1 from 'tiny-secp256k1';
import { faucet, getTransaction, initDefaultWallet } from "./bitcoin-client";
import { mintRune } from "./mint";
import { generateBlocks } from "./bitcoin-client";
import { RuneId } from "runelib";
import { toXOnly } from "bitcoinjs-lib/src/psbt/bip371";

bitcoin.initEccLib(tinySecp256k1);
const ECPair = ECPairFactory(tinySecp256k1);
const network = bitcoin.networks.regtest;

const WIF_PRIVATE_KEY = 'cSYFixQzjSrZ4b4LBT16Q7RXBk52DZ5cpJydE7DzuZS1RhzaXpEN';
const RUNE_ID_BLOCK = 18417;
const RUNE_ID_TX = 1;

async function main() {
  console.log('---- Minting phase ----');

	const keyPair = ECPair.fromWIF(WIF_PRIVATE_KEY, network);


	console.log('\n1. Initializing wallet and getting funds...');

	await initDefaultWallet();

	console.log('✅ Wallet initialized and funded');


	console.log('\n2. Getting p2tr address...');

	const { address: p2trAddress } = bitcoin.payments.p2tr({
		internalPubkey: toXOnly(Buffer.from(keyPair.publicKey)),
		network,
	});

	let faucetTxid = await faucet(p2trAddress!, 1);

	await generateBlocks(6);
	await new Promise(resolve => setTimeout(resolve, 2000));

	console.log('Output address:', p2trAddress);
	console.log('✅ Output address obtained');

	const faucetTransaction = await getTransaction(faucetTxid);

	const utxo = faucetTransaction.output[0];

	console.info('utxo:', utxo);

	console.info('✅ Faucet transaction obtained');

	console.log('\n3. Minting rune...');

	const mintRuneResponse = await mintRune({
		keyPair: ECPair.fromWIF(WIF_PRIVATE_KEY, network),
		utxo: { txid: faucetTxid, vout: 0, value: utxo.value },
		runeId: new RuneId(RUNE_ID_BLOCK, RUNE_ID_TX),
		outputAddress: p2trAddress!,
	});

	await generateBlocks(6);
	await new Promise(resolve => setTimeout(resolve, 2000));

	console.log('✅ Rune minted successfully!');

	console.log('\n4. Getting rune information from Titan...');

	const transaction = await getTransaction(mintRuneResponse.mintingUtxo.txid);
	
	let rune = transaction.output[1].runes[0];
	if (!rune || rune.amount === '0') {
		throw new Error('Rune not found in transaction output');
	}
	console.log('rune:', rune);

	console.log('✅ Rune found in transaction output');

	console.log('Minting response:', mintRuneResponse);

	console.log('\n✅ Rune minted successfully!');
}


main();