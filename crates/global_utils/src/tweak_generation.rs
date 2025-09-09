use bitcoin::hashes::Hash;
use bitcoin::key::{Parity, Secp256k1, TweakedPublicKey};
use bitcoin::secp256k1;
use bitcoin::secp256k1::rand::Rng;
use bitcoin::secp256k1::scalar::OutOfRangeError;
use bitcoin::secp256k1::{All, Scalar};
use std::sync::LazyLock;

pub type Nonce = [u8; 32];
pub struct TweakGenerator;

pub struct GeneratedTweakScalar {
    pub input_data: Vec<u8>,
    pub scalar: Scalar,
}

pub static SECP256K1_CURVE: LazyLock<Secp256k1<All>> = LazyLock::new(|| Secp256k1::new());

impl TweakGenerator {
    /// Generates nonce and adds it to input data to form Scalar with which Pubkey can be easily tweaked
    ///
    /// **Should never return error**
    pub fn generate_tweak_scalar(data: impl AsRef<[u8]>) -> Result<Scalar, OutOfRangeError> {
        let hash_of_bytes = bitcoin::hashes::sha256::Hash::hash(data.as_ref()).to_byte_array();
        let scalar = Scalar::from_be_bytes(hash_of_bytes)?;
        Ok(scalar)
    }

    pub fn generate_nonce() -> Nonce {
        let mut rand = bitcoin::key::rand::thread_rng();
        let mut nonce: Nonce = [0; 32];
        rand.fill(&mut nonce);
        nonce
    }

    pub fn tweak_pubkey(
        pubkey: secp256k1::PublicKey,
        scalar: &Scalar,
    ) -> Result<(TweakedPublicKey, Parity), secp256k1::Error> {
        let (tweaked_x, parity) = pubkey.add_exp_tweak(&*SECP256K1_CURVE, scalar)?.x_only_public_key();
        Ok((TweakedPublicKey::dangerous_assume_tweaked(tweaked_x), parity))
    }

    pub fn tweak_pubkey_package(
        pubkey: secp256k1::PublicKey,
        scalar: &Scalar,
    ) -> Result<(TweakedPublicKey, Parity), secp256k1::Error> {
        let (tweaked_x, parity) = pubkey.add_exp_tweak(&*SECP256K1_CURVE, scalar)?.x_only_public_key();
        Ok((TweakedPublicKey::dangerous_assume_tweaked(tweaked_x), parity))
    }
}
