import { ECPairFactory } from "ecpair";
import * as bitcoin from 'bitcoinjs-lib';
import * as tinySecp256k1 from 'tiny-secp256k1';
import { getTransaction, sendRawTransaction } from "../bitcoin-client";
import { generateBlocks } from "../bitcoin-client";
import { toXOnly } from "bitcoinjs-lib/src/psbt/bip371";

bitcoin.initEccLib(tinySecp256k1);
const ECPair = ECPairFactory(tinySecp256k1);
const network = bitcoin.networks.regtest;

const WIF_PRIVATE_KEY = 'cSYFixQzjSrZ4b4LBT16Q7RXBk52DZ5cpJydE7DzuZS1RhzaXpEN';


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
    const rawTransaction = `020000000001025775fcd23cecc9dadf3f6f0eb3ab5a7f062ae73aa82e31262a6da9bdc3098e080100000000fdffffff5775fcd23cecc9dadf3f6f0eb3ab5a7f062ae73aa82e31262a6da9bdc3098e080200000000fdffffff030000000000000000126a5d0f00f18f0101a0c21e010000a0c21e0270c9fa0200000000225120e1279c805cc9a755dc15287e58e38f3126807c8e35208f0e5b8d3ec06f10040770c9fa0200000000225120e1279c805cc9a755dc15287e58e38f3126807c8e35208f0e5b8d3ec06f100407014112931116da80768d62a4b77959291bafe2da473cf5cdba713f0b70d64afb4d18303d8d4c36964e49c05b0b7e6150b6964219cf1c5ff38c15831f276697e564a301014155784f6d6278edff039241d1d313545cd15b4f720f3f838dd70ca29453389ac05286f06fee42e23d12138122faa7f57e91412d14480796c6519332f9e58dbb380100000000`;
    const txid = await sendRawTransaction(rawTransaction);
    console.log('Txid:', txid);

    await generateBlocks(6);
    await new Promise(resolve => setTimeout(resolve, 2000));

    const transaction = await getTransaction(txid);
    console.log('Transaction:', transaction);
}

main2();