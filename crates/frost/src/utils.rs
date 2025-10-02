use crate::types::TweakBytes;
use bitcoin::Address;
use bitcoin::Network;
use bitcoin::TapNodeHash;
use bitcoin::hashes::Hash;
use bitcoin::secp256k1::Secp256k1;
use bitcoin::secp256k1::{Keypair, PublicKey};
use eyre::Result;
use frost_secp256k1_tr::keys::PublicKeyPackage;
use rand_core::{OsRng, RngCore};

pub fn convert_public_key_package(public_key_package: &PublicKeyPackage) -> Result<PublicKey> {
    let public_key_bytes = public_key_package.verifying_key().serialize()?;
    Ok(PublicKey::from_slice(&public_key_bytes)?)
}

pub fn generate_tweak_bytes() -> TweakBytes {
    let mut rng = OsRng;
    let mut nonce: TweakBytes = [0; 32];
    rng.fill_bytes(&mut nonce);
    nonce
}

pub fn get_tweaked_p2tr_address(public_key: PublicKey, tweak: TweakBytes, network: Network) -> Result<Address> {
    let ctx = Secp256k1::new();
    let (x_only_public_key, _) = public_key.x_only_public_key();

    let tap_node_hash =
        TapNodeHash::from_slice(&tweak).map_err(|e| eyre::eyre!("Failed to convert tweak to tap node hash: {}", e))?;

    let address = Address::p2tr(&ctx, x_only_public_key, Some(tap_node_hash), network);

    Ok(address)
}

pub fn generate_issuer_public_key() -> PublicKey {
    let mut rng = OsRng;
    let key_pair = Keypair::new(&Secp256k1::new(), &mut rng);
    key_pair.public_key()
}
