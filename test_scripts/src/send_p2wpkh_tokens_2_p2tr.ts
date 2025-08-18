import { ECPairFactory } from "ecpair";
import ecc from "@bitcoinerlab/secp256k1";
import { initEccLib, networks, payments, Psbt } from "bitcoinjs-lib";
import { signAndSend, toXOnly } from "./test_utils";

initEccLib(ecc);
const network = networks.regtest;
const wif_private_key = "cSYFixQzjSrZ4b4LBT16Q7RXBk52DZ5cpJydE7DzuZS1RhzaXpEN";
const ECPair = ECPairFactory(ecc);
const keyPair = ECPair.fromWIF(wif_private_key, network);
const utxo_tx_id =
  "363fd538f27d52b4487f7d19548c49422a1e9ebf63a29fa4480d28c5a43823ff";
const utxo_vout = 0;
const utxo_value = 5000000000;

// Get addresses
const p2wpkh = payments.p2wpkh({
  pubkey: Buffer.from(keyPair.publicKey),
  network,
});
const p2tr = payments.p2tr({
  internalPubkey: toXOnly(Buffer.from(keyPair.publicKey)),
  network,
});

// Build PSBT
const psbt = new Psbt({ network });
psbt.addInput({
  hash: utxo_tx_id,
  index: utxo_vout,
  witnessUtxo: { value: utxo_value, script: p2wpkh.output! },
});

const fee = 5000;
const change = utxo_value - 546 - fee;
psbt.addOutput({
  address: p2tr.address!, // change address
  value: 546,
});
psbt.addOutput({
  address: p2tr.address!, // change address
  value: change,
});

console.log(
  `P2WPKH Address: ${p2wpkh.address}, P2TR Address: ${p2tr.address}, UTXO: ${utxo_tx_id}, PSBT: ${psbt}`,
);

// Sign and finalize
await signAndSend(keyPair, psbt, [0, 1]);
//
// psbt.signAllInputs(keyPair);
// psbt.finalizeAllInputs();
// const tx = psbt.extractTransaction();
// console.log("Raw TX:", tx.toHex());
