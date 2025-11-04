import * as bitcoin from 'bitcoinjs-lib';
import { ECPairInterface } from 'ecpair';


export function toBitcoinSigner(keyPair: ECPairInterface): bitcoin.Signer {
	return {
		publicKey: Buffer.from(keyPair.publicKey),
		sign: (hash) => {
			const signature = keyPair.sign(hash);
			return Buffer.from(signature);
		},
		signSchnorr: (hash) => {
			const signature = keyPair.signSchnorr(hash);
			return Buffer.from(signature);
		},
	};
}
