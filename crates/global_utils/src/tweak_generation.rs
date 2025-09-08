use crate::logger::{LoggerGuard, init_logger};
use bitcoin::hashes::Hash;
use bitcoin::key::{Parity, Secp256k1, TweakedPublicKey, UntweakedPublicKey};
use bitcoin::secp256k1;
use bitcoin::secp256k1::scalar::OutOfRangeError;
use bitcoin::secp256k1::{All, PublicKey, Scalar};
use std::io::Read;
use std::sync::LazyLock;

pub struct TweakGeneration;

pub struct GeneratedTweakScalar {
    pub input_data: Vec<u8>,
    pub scalar: Scalar,
}

pub static SECP256K1_CURVE: LazyLock<Secp256k1<All>> = LazyLock::new(|| Secp256k1::new());

impl TweakGeneration {
    /// Generates nonce and adds it to input data to form Scalar with which Pubkey can be easily tweaked
    ///
    /// **Should never return error**
    pub fn generate_tweak_with_nonce(data: impl AsRef<[u8]>) -> Result<GeneratedTweakScalar, OutOfRangeError> {
        let nonce = Scalar::random();
        let mut predefined_data = data.as_ref().to_vec();
        predefined_data.extend_from_slice(&nonce.to_be_bytes());
        let hash_of_bytes = bitcoin::hashes::sha256::Hash::hash(&predefined_data).to_byte_array();
        let scalar = Scalar::from_be_bytes(hash_of_bytes)?;
        Ok(GeneratedTweakScalar {
            input_data: predefined_data,
            scalar,
        })
    }

    pub fn tweak_pubkey(
        pubkey: secp256k1::PublicKey,
        scalar: &Scalar,
    ) -> Result<(TweakedPublicKey, Parity), secp256k1::Error> {
        let (tweaked_x, parity) = pubkey.add_exp_tweak(&*SECP256K1_CURVE, scalar)?.x_only_public_key();
        Ok((TweakedPublicKey::dangerous_assume_tweaked(tweaked_x), parity))
    }
}
