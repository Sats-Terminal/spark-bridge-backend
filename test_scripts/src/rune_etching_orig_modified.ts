import {
  crypto,
  initEccLib,
  networks,
  payments,
  Psbt,
  script,
  Signer as BTCSigner,
} from "bitcoinjs-lib";
import { Taptree } from "bitcoinjs-lib/src/types";
import { ECPairAPI, ECPairFactory } from "ecpair";
import ecc from "@bitcoinerlab/secp256k1";
import axios, { AxiosResponse } from "axios";
import {
  Etching,
  EtchInscription,
  none,
  Range,
  Rune,
  Runestone,
  some,
  Terms,
} from "runelib";
import { call_rpc } from "../titan_test/rpc_utils";
import { signAndSend, toXOnly } from "./test_utils";

initEccLib(ecc as any);
// declare const window: any;
const ECPair: ECPairAPI = ECPairFactory(ecc);
const network = networks.regtest;

// mint: http://bridge.scrypt.io:8888/rune/BESTSCRYPTMINT
const name = "CCCCCCCCCCCCCCCCCCNH";
const wif_private_key = "cSYFixQzjSrZ4b4LBT16Q7RXBk52DZ5cpJydE7DzuZS1RhzaXpEN";
const keyPair = ECPair.fromWIF(wif_private_key, network);
const utxo_tx_id =
  "6eb70a7fbc44f63fd0fe24291bc007a5ee4d44e759af21aec467fda7fec3dab3";
const utxo_vout = 0;
const utxo_value = 1250000000;

async function etching() {
  const { address: p2wpkh_address } = payments.p2wpkh({
    pubkey: Buffer.from(keyPair.publicKey),
    network,
  });
  const { address: p2tr_address } = payments.p2tr({
    pubkey: toXOnly(Buffer.from(keyPair.publicKey)),
    network,
  });

  console.log("p2wpkh address: ", p2wpkh_address);
  console.log("p2tr address: ", p2tr_address);
  // stop here and get address to insert into PUB_ADDR
  // process.exit(0);

  const ins = new EtchInscription();

  ins.setContent("text/plain", Buffer.from("scrypt is best", "utf-8"));
  ins.setRune(name);

  const etching_script_asm = `${toXOnly(
    Buffer.from(keyPair.publicKey),
  ).toString("hex")} OP_CHECKSIG`;
  const etching_script = Buffer.concat([
    script.fromASM(etching_script_asm),
    ins.encipher(),
  ]);

  const scriptTree: Taptree = {
    output: etching_script,
  };

  const script_p2tr = payments.p2tr({
    internalPubkey: toXOnly(Buffer.from(keyPair.publicKey)),
    scriptTree,
    network,
  });

  const etching_redeem = {
    output: etching_script,
    redeemVersion: 192,
  };

  const etching_p2tr = payments.p2tr({
    internalPubkey: toXOnly(Buffer.from(keyPair.publicKey)),
    scriptTree,
    redeem: etching_redeem,
    network,
  });

  const address = script_p2tr.address ?? "";
  console.log("send coin to address", address);

  console.log(`Using UTXO ${utxo_tx_id}:${utxo_vout}`);

  const psbt = new Psbt({ network });

  psbt.addInput({
    hash: utxo_tx_id,
    index: utxo_vout,
    witnessUtxo: { value: utxo_value, script: script_p2tr.output! },
    tapLeafScript: [
      {
        leafVersion: etching_redeem.redeemVersion,
        script: etching_redeem.output,
        controlBlock: etching_p2tr.witness![etching_p2tr.witness!.length - 1],
      },
    ],
  });

  const rune = Rune.fromName(name);

  const terms = new Terms(
    1000,
    10000,
    new Range(none(), none()),
    new Range(none(), none()),
  );

  const etching = new Etching(
    some(1),
    some(0),
    some(rune),
    none(),
    some("$"),
    some(terms),
    true,
  );

  const stone = new Runestone([], some(etching), none(), none());

  psbt.addOutput({
    script: stone.encipher(),
    value: 0,
  });

  const fee = 5000;

  const change = utxo_value - 546 - fee;

  psbt.addOutput({
    address: p2tr_address!, // change address
    value: 546,
  });

  psbt.addOutput({
    address: p2tr_address!, // change address
    value: change,
  });

  await signAndSend(keyPair, psbt, [0]);
}

// main
etching();
