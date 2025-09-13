import { ECPairFactory } from "ecpair";
import * as bitcoin from 'bitcoinjs-lib';
import * as tinySecp256k1 from 'tiny-secp256k1';
import { faucet, getTransaction, initDefaultWallet, sendRawTransaction } from "../bitcoin-client";
import { mintRune } from "../mint";
import { generateBlocks } from "../bitcoin-client";
import { RuneId } from "runelib";
import { toXOnly } from "bitcoinjs-lib/src/psbt/bip371";

bitcoin.initEccLib(tinySecp256k1);
const ECPair = ECPairFactory(tinySecp256k1);
const network = bitcoin.networks.regtest;

const WIF_PRIVATE_KEY = 'cSYFixQzjSrZ4b4LBT16Q7RXBk52DZ5cpJydE7DzuZS1RhzaXpEN';
const RUNE_ID_BLOCK = 18417;
const RUNE_ID_TX = 1;


async function main1() {
    const keyPair = ECPair.fromWIF(WIF_PRIVATE_KEY, network);

    const { address: p2trAddress } = bitcoin.payments.p2tr({
        internalPubkey: toXOnly(Buffer.from(keyPair.publicKey)),
        network,
    });
    
    console.log('P2TR address:', p2trAddress);

    const transaction = await getTransaction("4bed4e12a27e0786d0f236811fc9a6cd372a9d11ff0cdf34d57277db1ed65068");

    console.log('Transaction:', transaction);
}

async function main2() {
    const rawTransaction = `020000000001024bed4e12a27e0786d0f236811fc9a6cd372a9d11ff0cdf34d57277db1ed650680100000000fdffffff4bed4e12a27e0786d0f236811fc9a6cd372a9d11ff0cdf34d57277db1ed650680200000000fdffffff030000000000000000126a5d0f00f18f0101a0c21e010000a0c21e0270c9fa0200000000225120e1279c805cc9a755dc15287e58e38f3126807c8e35208f0e5b8d3ec06f10040770c9fa0200000000225120e1279c805cc9a755dc15287e58e38f3126807c8e35208f0e5b8d3ec06f1004070141c85b1b43cd3dd5f2ab11811c210c842aa1d28c301520c87147080bb4a14d5a1840521820ca6801ed299855aba394365e09ef20a2fe2672acb01e5b07d1521d790101416632b77074df0552cf3b246ea6cd8c054c67d01b0f01b0bbffac251362cd279b98b251fe7db7702ac5e0173558b011fd147744a58a7c9abf51fe1ab03e268cd00100000000`;
    const txid = await sendRawTransaction(rawTransaction);
    console.log('Txid:', txid);

    await generateBlocks(6);

    const transaction = await getTransaction(txid);
    console.log('Transaction:', transaction);
}

main1();