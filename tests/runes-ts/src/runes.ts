import * as tinySecp256k1 from 'tiny-secp256k1';
import { ECPairFactory } from 'ecpair';
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

// Initialize ECC library
bitcoin.initEccLib(tinySecp256k1);

const ECPair = ECPairFactory(tinySecp256k1);
const network = bitcoin.networks.regtest;

export interface EtchRuneParams {
  rune: Rune;
  privateKey: string; // WIF format
  utxo: {
    txid: string;
    vout: number;
    value: number;
  };
  symbol?: string;
  divisibility?: number;
}

export interface CreateRuneAddressResponse {
	address: string;
	script_p2tr: any;
	etching_redeem: any;
	etching_p2tr: any;
}

export async function createRuneAddress(privateKey: string, runeName: string): Promise<CreateRuneAddressResponse> {
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
  
  const script_p2tr = bitcoin.payments.p2tr({
    internalPubkey: toXOnly(Buffer.from(keyPair.publicKey)),
    scriptTree,
    network,
  });
  
  const etching_redeem = {
    output: etching_script,
    redeemVersion: 192,
  };
  
  const etching_p2tr = bitcoin.payments.p2tr({
    internalPubkey: toXOnly(Buffer.from(keyPair.publicKey)),
    scriptTree,
    redeem: etching_redeem,
    network,
  });
  
  return {
    address: script_p2tr.address ?? '',
    script_p2tr,
    etching_redeem,
    etching_p2tr
  };
}

export async function etchRune(params: EtchRuneParams): Promise<RuneId> {
  const {
    rune,
    privateKey,
    utxo,
    symbol = '$',
    divisibility = 0,
  } = params;
	const keyPair = ECPair.fromWIF(privateKey, network);

  // Create the same P2TR address with tapscript
  const createRuneAddressResponse = await createRuneAddress(privateKey, rune.name);
  
  console.log('Etching address:', createRuneAddressResponse.address);

  // Create PSBT
  const psbt = new bitcoin.Psbt({ network });

  // Add input (P2TR)
  psbt.addInput({
    hash: utxo.txid,
    index: utxo.vout,
    witnessUtxo: { value: utxo.value, script: createRuneAddressResponse.script_p2tr.output! },
    tapLeafScript: [
      {
        leafVersion: createRuneAddressResponse.etching_redeem.redeemVersion,
        script: createRuneAddressResponse.etching_redeem.output,
        controlBlock: createRuneAddressResponse.etching_p2tr.witness![createRuneAddressResponse.etching_p2tr.witness!.length - 1],
      },
    ],
  });

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

  // Add inscription output
  psbt.addOutput({
    address: createRuneAddressResponse.address!,
    value: 546, // Dust limit
  });

  // Add change output
  const fee = 5000;
  const dustLimit = 546;
  const totalRequired = dustLimit + fee;
  
  // Validate UTXO has sufficient funds
  if (utxo.value < totalRequired) {
    throw new Error(`Insufficient funds: UTXO has ${utxo.value} satoshis but needs at least ${totalRequired} satoshis (${dustLimit} dust + ${fee} fee). Please ensure the address has sufficient BTC.`);
  }
  
  const change = utxo.value - dustLimit - fee;
  
  // Only add change output if there's actually change to return
  if (change > 0) {
    psbt.addOutput({
      address: createRuneAddressResponse.address!,
      value: change,
    });
  }

  // Sign and send
  const txid = await signAndSend(keyPair, psbt, [0]);

  // Generate a block to confirm the transaction
  await generateBlocks(6);

  console.log('Rune etching completed!');

  await new Promise(resolve => setTimeout(resolve, 2000));

  return await getRuneId(txid);
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
