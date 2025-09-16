use bitcoin::secp256k1::PublicKey;
use eyre::Result;
use frost_secp256k1_tr::keys::PublicKeyPackage;
use crate::types::Nonce;
use rand_core::{RngCore, OsRng};
use bitcoin::Network;
use bitcoin::Address;
use bitcoin::secp256k1::Secp256k1;
use bitcoin::TapNodeHash;
use bitcoin::hashes::Hash;

pub fn convert_public_key_package(public_key_package: &PublicKeyPackage) -> Result<PublicKey> {
    let public_key_bytes = public_key_package.verifying_key().serialize()?;
    Ok(PublicKey::from_slice(&public_key_bytes)?)
}

pub fn generate_nonce() -> Nonce {
    let mut rng = OsRng;
    let mut nonce: Nonce = [0; 32];
    rng.fill_bytes(&mut nonce);
    nonce
}

pub fn get_address(public_key: PublicKey, tweak: Nonce, network: Network) -> Result<Address> {
    let ctx = Secp256k1::new();
    let (x_only_public_key, _) = public_key.x_only_public_key();

    let tap_node_hash = TapNodeHash::from_slice(&tweak).map_err(|e| eyre::eyre!("Failed to convert tweak to tap node hash: {}", e))?;
    
    let address = Address::p2tr(&ctx, x_only_public_key, Some(tap_node_hash), network);

    Ok(address)
}
