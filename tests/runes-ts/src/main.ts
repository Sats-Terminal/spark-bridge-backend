import { Rune, RuneId } from 'runelib';
import { etchRune, createRunePayments } from './runes';
import { initDefaultWallet, generateBlocks, faucet, getAddressData, getRune, getRuneId, getTransaction } from './bitcoin-client';
import * as bitcoin from 'bitcoinjs-lib';
import * as tinySecp256k1 from 'tiny-secp256k1';
import { AddressTxOut } from '@titanbtcio/sdk';

// Initialize ECC library
bitcoin.initEccLib(tinySecp256k1);

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

	// Generate a random rune name (8 characters by default)
	const runeName = generateRandomRuneName(15);
	console.log('🚀 Starting rune etching process...\n');
	console.log('Generated rune name:', runeName);

	// 1. Initialize wallet and get some funds
	console.log('1. Initializing wallet and getting funds...');
	await initDefaultWallet();
	console.log('✅ Wallet initialized and funded\n');

	// 2. Create P2TR address with tapscript for rune etching
	console.log('2. Creating P2TR address with tapscript and fauceting BTC...');
	
	// Create the same P2TR address that will be used in etching
	const createRuneAddressResponse = await createRunePayments(WIF_PRIVATE_KEY, runeName);
	const p2trAddress = createRuneAddressResponse.p2trOutput.address;
	console.log('P2TR address with tapscript:', p2trAddress);
	
	// Faucet BTC to the P2TR address (more than needed for transaction)
	console.log('Fauceting 1 BTC to address:', p2trAddress);
	const faucetTxid = await faucet(p2trAddress!, 1); // 1 BTC = 100,000,000 satoshis
	console.log('Faucet transaction ID:', faucetTxid);
	console.log('✅ BTC fauceted to P2TR address\n');

	// 3. Generate blocks to confirm the transaction
	console.log('3. Generating blocks to confirm transaction...');
	await generateBlocks(6);
	console.log('✅ Blocks generated\n');

	await new Promise(resolve => setTimeout(resolve, 1000)); // wait for 1 second

	// 4. Get UTXOs for the P2TR address
	console.log('4. Getting UTXOs...');
	const addressData = await getAddressData(p2trAddress!);
	const utxos = addressData.outputs || [];
	console.log('Available UTXOs from Titan:', utxos.length);

	if (utxos.length !== 1) {
		throw new Error('Expected 1 UTXO, got ' + utxos.length);
	}
	const utxo = utxos[0];

	// 5. Create a rune
	console.log('5. Creating rune...');
	const rune = Rune.fromName(runeName);
	console.log('Rune name:', rune.name);
	console.log('✅ Rune created\n');

	// 6. Etch the rune
	console.log('6. Etching rune...');
	const etchRuneResponse = await etchRune({
		rune: rune,
		privateKey: WIF_PRIVATE_KEY,
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

	const runeId = await getRuneId(etchRuneResponse.changeUtxo.txid);
	
	console.log(`Rune ID: ${runeId.block}:${runeId.idx}`);
	console.log('✅ Rune etched successfully!');

	// 7. Get rune information from Titan
	console.log('\n7. Getting rune information from Titan...');
	// Wait a bit for the transaction to be processed
	await new Promise(resolve => setTimeout(resolve, 2000));
	
	// Get rune info from Titan
	let runeResponse = await getRune(runeId);
	console.log('Rune Response:', runeResponse);

	console.log('\n✅ Rune etching process completed successfully!\n\n');


	console.log("---- Minting phase ----");


}

// Run the main function
main();
