import { Rune, RuneId } from 'runelib';
import { etchRune, createRuneAddress } from './runes';
import { initDefaultWallet, generateBlocks, faucet, getAddressData, getRune } from './bitcoin-client';
import * as bitcoin from 'bitcoinjs-lib';
import * as tinySecp256k1 from 'tiny-secp256k1';

// Initialize ECC library
bitcoin.initEccLib(tinySecp256k1);

// WIF private key for regtest
const WIF_PRIVATE_KEY = 'cSYFixQzjSrZ4b4LBT16Q7RXBk52DZ5cpJydE7DzuZS1RhzaXpEN';

async function main() {
	const runeName = 'MAYBETRYLONGRUNENAME';
	console.log('🚀 Starting rune etching process...\n');

	// 1. Initialize wallet and get some funds
	console.log('1. Initializing wallet and getting funds...');
	await initDefaultWallet();
	console.log('✅ Wallet initialized and funded\n');

	// 2. Create P2TR address with tapscript for rune etching
	console.log('2. Creating P2TR address with tapscript and fauceting BTC...');
	
	// Create the same P2TR address that will be used in etching
	const { address: p2tr_address } = await createRuneAddress(WIF_PRIVATE_KEY, runeName);
	console.log('P2TR address with tapscript:', p2tr_address);
	
	// Faucet BTC to the P2TR address (more than needed for transaction)
	console.log('Fauceting 0.1 BTC to address:', p2tr_address);
	const faucetTxid = await faucet(p2tr_address!, 0.1); // 0.1 BTC = 10,000,000 satoshis
	console.log('Faucet transaction ID:', faucetTxid);
	console.log('✅ BTC fauceted to P2TR address\n');

	// 3. Generate blocks to confirm the transaction
	console.log('3. Generating blocks to confirm transaction...');
	await generateBlocks(6);
	console.log('✅ Blocks generated\n');

	await new Promise(resolve => setTimeout(resolve, 1000)); // wait for 1 second

	// 4. Get UTXOs for the P2TR address
	console.log('4. Getting UTXOs...');
	const addressData = await getAddressData(p2tr_address!);
	const utxos = addressData.outputs || [];
	console.log('Available UTXOs from Titan:', utxos.length);
	
	// Debug: Show all UTXOs
	utxos.forEach((utxo, index) => {
		console.log(`UTXO ${index}:`, {
			txid: utxo.txid,
			vout: utxo.vout,
			value: utxo.value,
			valueInBTC: (utxo.value / 100000000).toFixed(8)
		});
	});
	
	if (utxos.length === 0) {
		throw new Error('No UTXOs available for etching');
	}

	// Find the UTXO with the highest value
	const utxo = utxos.reduce((max, current) => current.value > max.value ? current : max);
	console.log('Using UTXO with highest value:', utxo);
	console.log('UTXO value in satoshis:', utxo.value);
	console.log('UTXO value in BTC:', (utxo.value / 100000000).toFixed(8));
	console.log('✅ UTXO selected\n');

	// 5. Create a rune
	console.log('5. Creating rune...');
	const rune = Rune.fromName(runeName);
	console.log('Rune name:', rune.name);
	console.log('✅ Rune created\n');

	// 6. Etch the rune
	console.log('6. Etching rune...');
	const runeId: RuneId = await etchRune({
		rune: rune,
		privateKey: WIF_PRIVATE_KEY,
		utxo: {
			txid: utxo.txid,
			vout: utxo.vout,
			value: utxo.value, // Already in satoshis
		},
		symbol: '$',
		divisibility: 3,
	});
	
	console.log('✅ Rune etched successfully!');
	console.log(`Rune ID: ${runeId.block}:${runeId.idx}`);

	// 7. Get rune information from Titan
	console.log('\n7. Getting rune information from Titan...');
	// Wait a bit for the transaction to be processed
	await new Promise(resolve => setTimeout(resolve, 2000));
	
	// Get rune info from Titan
	let runeResponse = await getRune(runeId);
	console.log('Rune Response:', runeResponse);

	console.log('\n🎉 Rune etching process completed successfully!');

}

// Run the main function
main();
