import * as tinySecp256k1 from 'tiny-secp256k1';
import { ECPairFactory, ECPairInterface } from 'ecpair';
import * as bitcoin from 'bitcoinjs-lib';
import {
  Etching,
  EtchInscription,
  none,
  Runestone,
  some,
  Rune,
  RuneId,
} from 'runelib';
import { sendRawTransaction, generateBlocks, getRuneId } from './bitcoin-client';
import { Payment } from 'bitcoinjs-lib';

// Initialize ECC library
bitcoin.initEccLib(tinySecp256k1);

const ECPair = ECPairFactory(tinySecp256k1);
const network = bitcoin.networks.regtest;

export interface CreateRuneAddressResponse {
	p2trOutput: Payment;
	p2trInput: Payment;
}

export async function createRunePayments(privateKey: string, runeName: string): Promise<CreateRuneAddressResponse> {
  const keyPair = ECPair.fromWIF(privateKey, network);
  
  // Create etching inscription
  const ins = new EtchInscription();
  ins.setContent('text/plain', Buffer.from('scrypt is best', 'utf-8'));
  ins.setRune(runeName);
  
  // Create etching script
  const etching_script_asm = `${toXOnly(Buffer.from(keyPair.publicKey)).toString('hex')} OP_CHECKSIG`;
  const etching_script = Buffer.concat([
    bitcoin.script.fromASM(etching_script_asm),
    ins.encipher(),
  ]);
  
  // Create script tree for P2TR
  const scriptTree: any = {
    output: etching_script,
  };
  
  const p2trOutput = bitcoin.payments.p2tr({
    internalPubkey: toXOnly(Buffer.from(keyPair.publicKey)),
    scriptTree,
    network,
  });
  
  const etching_redeem = {
    output: etching_script,
    redeemVersion: 192,
  };

	const p2trInput = bitcoin.payments.p2tr({
		internalPubkey: toXOnly(Buffer.from(keyPair.publicKey)),
		scriptTree,
		redeem: etching_redeem,
		network,
	});
  
  return {
    p2trOutput,
		p2trInput,
  };
}

export interface EtchRuneParams {
  runeName: string;
  keyPair: ECPairInterface
  utxo: {
    txid: string;
    vout: number;
    value: number;
		p2trInput: Payment;
  };
  symbol?: string;
  divisibility?: number;
}

export interface EtchRuneResponse {
  changeUtxo: {
		txid: string;
		vout: number;
		value: number;
	},
	etchingUtxo: {
		txid: string;
		vout: number;
		value: number;
	},
}

export async function etchRune(params: EtchRuneParams): Promise<EtchRuneResponse> {
  const {
    runeName,
    keyPair,
    utxo,
    symbol = '$',
    divisibility = 0,
  } = params;
  // Create PSBT
  const psbt = new bitcoin.Psbt({ network });

  // Add input (P2TR)
  psbt.addInput({
    hash: utxo.txid,
    index: utxo.vout,
    witnessUtxo: { value: utxo.value, script: utxo.p2trInput.output! },
    tapLeafScript: [
      {
        leafVersion: utxo.p2trInput.redeemVersion!,
        script: utxo.p2trInput.redeem!.output!,
        controlBlock: utxo.p2trInput.witness![utxo.p2trInput.witness!.length - 1],
      },
    ],
  });

	const rune = Rune.fromName(runeName);

  // Create etching
  const etching = new Etching(
    some(divisibility),
    some(0),
    some(rune),
    none(),
    some(symbol),
    none(),
    true, // turbo
  );

  // Create runestone
  const stone = new Runestone([], some(etching), none(), none());

  // Add runestone output
  psbt.addOutput({
    script: stone.encipher(),
    value: 0,
  });

	const dustLimit = 546;

  // Add inscription output
  psbt.addOutput({
    address: utxo.p2trInput.address!,
    value: dustLimit, // Dust limit
  });

  // Add change output
  const fee = 5000;
  const totalRequired = dustLimit + fee;
  
  // Validate UTXO has sufficient funds
  if (utxo.value < totalRequired) {
    throw new Error(`Insufficient funds: UTXO has ${utxo.value} satoshis but needs at least ${totalRequired} satoshis (${dustLimit} dust + ${fee} fee). Please ensure the address has sufficient BTC.`);
  }
  
  const change = utxo.value - dustLimit - fee;

	if (change < dustLimit) {
		throw new Error(`Change is less than dust limit: ${change} < ${dustLimit}`);
	}
  
	psbt.addOutput({
		address: utxo.p2trInput.address!,
		value: change,
	});

  // Sign and send
  const txid = await signAndSend(keyPair, psbt, [0]);

  await new Promise(resolve => setTimeout(resolve, 2000));

  return {
    changeUtxo: {
      txid: txid,
      vout: 2,
      value: change,
    },
		etchingUtxo: {
			txid: txid,
			vout: 1,
			value: dustLimit,
		},
  };
}

// Helper function to convert public key to x-only
function toXOnly(pubkey: Buffer): Buffer {
  return pubkey.subarray(1, 33);
}

// Helper function to sign and send PSBT
async function signAndSend(keyPair: any, psbt: bitcoin.Psbt, taprootIndexes: number[] = []) {
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
