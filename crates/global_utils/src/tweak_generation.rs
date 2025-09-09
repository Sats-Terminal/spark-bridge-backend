use bitcoin::hashes::{FromSliceError, Hash, sha256};
use bitcoin::key::{Parity, Secp256k1, TapTweak, TweakedPublicKey};
use bitcoin::secp256k1::rand::Rng;
use bitcoin::secp256k1::{All, PublicKey, Scalar};
use bitcoin::{TapNodeHash, secp256k1};
use frost_secp256k1_tr::VerifyingKey;
use frost_secp256k1_tr::keys::{PublicKeyPackage, Tweak};
use std::sync::LazyLock;

pub type Nonce = [u8; 32];
pub type HashData = [u8; 32];
pub type RuneId = String;
pub struct TweakGenerator;

pub struct GeneratedTweakScalar {
    pub input_data: Vec<u8>,
    pub scalar: Scalar,
}

pub static SECP256K1_CURVE: LazyLock<Secp256k1<All>> = LazyLock::new(|| Secp256k1::new());

impl TweakGenerator {
    pub fn generate_nonce() -> Nonce {
        let mut rand = bitcoin::key::rand::thread_rng();
        let mut nonce: Nonce = [0; 32];
        rand.fill(&mut nonce);
        nonce
    }

    /// Function uses Bitcoin hash algorithm to hash values
    pub fn hash(data: impl AsRef<[u8]>) -> HashData {
        sha256::Hash::hash(data.as_ref()).to_byte_array()
    }

    pub fn tweak_btc_pubkey(
        secp: &Secp256k1<All>,
        pubkey: PublicKey,
        hashed_bytes: impl AsRef<[u8]>,
    ) -> Result<(TweakedPublicKey, Parity), FromSliceError> {
        let (tweaked_pubkey, parity) = pubkey
            .x_only_public_key()
            .0
            .tap_tweak(&secp, Some(TapNodeHash::from_slice(hashed_bytes.as_ref())).transpose()?);
        Ok((tweaked_pubkey, parity))
    }

    pub fn tweak_pubkey_package(
        public_key_package: PublicKeyPackage,
        hashed_bytes: impl AsRef<[u8]>,
    ) -> PublicKeyPackage {
        public_key_package.clone().tweak(Some(hashed_bytes.as_ref()))
    }

    pub fn tweaked_verifying_key_to_tweaked_pubkey(
        verifying_key: &VerifyingKey,
    ) -> anyhow::Result<((TweakedPublicKey, Parity))> {
        let btc_pubkey = PublicKey::from_slice(&verifying_key.serialize()?)?;
        let (tweaked_x, parity) = btc_pubkey.x_only_public_key();
        Ok((TweakedPublicKey::dangerous_assume_tweaked(tweaked_x), parity))
    }
    pub fn generate_byte_seq_rune_spark(
        pubkey: secp256k1::PublicKey,
        rune_id: RuneId,
        amount: u64,
        nonce: Nonce,
    ) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(pubkey.to_string().as_bytes());
        data.extend_from_slice(rune_id.as_bytes());
        data.extend_from_slice(&amount.to_be_bytes());
        data.extend_from_slice(&nonce);
        data
    }

    pub fn generate_byte_seq_spark_rune(nonce: Nonce) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&nonce);
        data
    }
}
