import { Rune } from 'runelib';
import { etchRune, createRunePayments } from './runes';
import { initDefaultWallet, generateBlocks, faucet, getAddressData, getRune, getRuneId } from './bitcoin-client';
import * as bitcoin from 'bitcoinjs-lib';
import * as tinySecp256k1 from 'tiny-secp256k1';
import { ECPairFactory } from 'ecpair';

// Initialize ECC library
bitcoin.initEccLib(tinySecp256k1);
const ECPair = ECPairFactory(tinySecp256k1);
const network = bitcoin.networks.regtest;

// WIF private key for regtest
const WIF_PRIVATE_KEY = 'cSYFixQzjSrZ4b4LBT16Q7RXBk52DZ5cpJydE7DzuZS1RhzaXpEN';
const SATOSHI_AMOUNT = 100_000_000;

// Generate random uppercase string for rune name
function generateRandomRuneName(length: number): string {
	const letters = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ';
	let result = '';
	for (let i = 0; i < length; i++) {
		result += letters.charAt(Math.floor(Math.random() * letters.length));
	}
	return result;
}

async function main() {
	console.log("---- Etching phase ----");

	const keyPair = ECPair.fromWIF(WIF_PRIVATE_KEY, network);
	const runeName = generateRandomRuneName(15);

	console.log('Generated rune name:', runeName);


	console.log('\n1. Initializing wallet and getting funds...');

	await initDefaultWallet();

	console.log('✅ Wallet initialized and funded');


	console.log('\n2. Creating P2TR address with tapscript and fauceting BTC...');

	const createRuneAddressResponse = await createRunePayments(WIF_PRIVATE_KEY, runeName);
	const p2trAddress = createRuneAddressResponse.p2trOutput.address;

	console.log('P2TR address with tapscript:', p2trAddress);
	

	console.log('\n3. Fauceting 1 BTC to p2tr address');

	await faucet(p2trAddress!, 1); // 1 BTC = 100,000,000 satoshis
	await generateBlocks(6);
	await new Promise(resolve => setTimeout(resolve, 1000));

	console.log('✅ BTC fauceted to P2TR address');


	console.log('\n4. Getting UTXOs...');

	const addressData = await getAddressData(p2trAddress!);
	const utxos = addressData.outputs || [];

	console.debug('Available UTXOs from Titan:', utxos.length);

	if (utxos.length !== 1) {
		throw new Error('Expected 1 UTXO, got ' + utxos.length);
	}
	const utxo = utxos[0];

	console.debug('Selected UTXO:', utxo);
	console.info('✅ UTXO selected');


	console.log('\n5. Etching rune...');

	const etchRuneResponse = await etchRune({
		runeName: runeName,
		keyPair: keyPair,
		utxo: {
			txid: utxo.txid,
			vout: utxo.vout,
			value: utxo.value, // Already in satoshis
			p2trInput: createRuneAddressResponse.p2trInput,
		},
		symbol: '$',
		divisibility: 3,
	});

	await generateBlocks(6);
	await new Promise(resolve => setTimeout(resolve, 2000));


	console.log('\n6. Getting rune information from Titan...');	

	const runeId = await getRuneId(etchRuneResponse.changeUtxo.txid);
	console.log(`Rune ID: ${runeId.block}:${runeId.idx}`);

	let runeResponse = await getRune(runeId);
	console.log('Rune Response:', runeResponse);

	console.log('\n✅ Rune etching process completed successfully!');

}

// Run the main function
main();
