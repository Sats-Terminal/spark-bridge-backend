use bitcoin::secp256k1::PublicKey;
use eyre::Result;
use frost_secp256k1_tr::keys::PublicKeyPackage;

pub fn convert_public_key_package(public_key_package: PublicKeyPackage) -> Result<PublicKey> {
    let public_key_bytes = public_key_package.verifying_key().serialize()?;
    Ok(PublicKey::from_slice(&public_key_bytes)?)
}
