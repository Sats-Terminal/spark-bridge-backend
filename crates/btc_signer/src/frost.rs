use async_trait::async_trait;
use std::collections::BTreeMap;

use crate::{
    api::Signer,
    config::SignerConfig,
    errors::{Result, SignerError},
};
use frost_secp256k1 as frost;
use frost_secp256k1::{
    keys::dkg::{round1, round2},
    keys::{KeyPackage, PublicKeyPackage, SecretShare},
    round1::SigningNonces,
    round2::SignatureShare,
    Identifier,
    SigningPackage,
};
use rand::rngs::OsRng;
use secp256k1::PublicKey;

#[derive(Debug, Clone)]
struct DkgState {
    round1_secret_package: Option<Vec<u8>>,
    round2_secret_package: Option<Vec<u8>>,
}

pub struct FrostSigner {
    config: SignerConfig,
    key_package: Option<KeyPackage>,
    public_key_package: Option<PublicKeyPackage>,
    last_signing_nonces: Option<SigningNonces>,
    threshold: u16,
    total_participants: u16,
    dkg_state: Option<DkgState>,
}

impl FrostSigner {
    pub fn new(config: SignerConfig) -> Self {
        Self {
            config,
            key_package: None,
            public_key_package: None,
            last_signing_nonces: None,
            threshold: 2,
            total_participants: 3,
            dkg_state: None,
        }
    }

    pub fn set_parameters(&mut self, threshold: u16, total_participants: u16) {
        self.threshold = threshold;
        self.total_participants = total_participants;
    }

    pub fn generate_keys_with_trusted_dealer<R: rand_core::RngCore + rand_core::CryptoRng>(
        threshold: u16,
        rng: &mut R,
    ) -> Result<(BTreeMap<Identifier, KeyPackage>, PublicKeyPackage)> {
        let (shares_map, pubpkg): (BTreeMap<Identifier, SecretShare>, PublicKeyPackage) =
            frost::keys::generate_with_dealer(
                threshold,
                threshold,
                frost::keys::IdentifierList::Default,
                rng,
            ).map_err(|e| SignerError::Internal(format!("frost dealer gen failed: {e}")))?;

        let mut key_packages: BTreeMap<Identifier, KeyPackage> = BTreeMap::new();
        for (id, secret_share) in shares_map {
            let kp = KeyPackage::try_from(secret_share)
                .map_err(|e| SignerError::Internal(format!("KeyPackage try_from failed: {e}")))?;
            key_packages.insert(id, kp);
        }

        Ok((key_packages, pubpkg))
    }
}

#[async_trait]
impl Signer for FrostSigner {
    fn get_participant_id(&self) -> &Identifier {
        &self.config.participant_id
    }

    async fn get_public_key_share(&self) -> Result<secp256k1::PublicKey> {
        if let Some(kp) = &self.key_package {
            let vs = kp.verifying_share();
            let vs_bytes = vs.serialize()
                .map_err(|e| SignerError::Internal(format!("verifying_share serialize failed: {e}")))?;
            let pk = PublicKey::from_slice(&vs_bytes)
                .map_err(|e| SignerError::Internal(format!("secp public from slice failed: {e}")))?;
            return Ok(pk);
        }

        if let Some(pubpkg) = &self.public_key_package {
            let id = *self.get_participant_id();
            let vs = pubpkg.verifying_shares().get(&id)
                .ok_or_else(|| SignerError::Internal("verifying share for participant not found".to_string()))?;
            let vs_bytes = vs.serialize()
                .map_err(|e| SignerError::Internal(format!("share serialize failed: {e}")))?;
            let pk = PublicKey::from_slice(&vs_bytes)
                .map_err(|e| SignerError::Internal(format!("secp public from slice failed: {e}")))?;
            return Ok(pk);
        }

        Err(SignerError::Internal("No key package or public key package available".to_string()))
    }

    async fn dkg_round_1(&mut self) -> Result<round1::Package> {
        let identifier: Identifier = self.config.participant_id;

        let mut rng = OsRng;

        let (secret_package, package) = frost::keys::dkg::part1(
            identifier,
            self.total_participants,
            self.threshold,
            &mut rng,
        ).map_err(|e| SignerError::Internal(format!("DKG round1 failed: {e}")))?;

        let secret_bytes = serde_json::to_vec(&secret_package)
            .map_err(|e| SignerError::Internal(format!("serialize secret package: {e}")))?;

        self.dkg_state = Some(DkgState {
            round1_secret_package: Some(secret_bytes),
            round2_secret_package: None,
        });

        Ok(package)
    }

    async fn dkg_round_2(
        &mut self,
        round1_packages: &BTreeMap<Identifier, round1::Package>,
    ) -> Result<BTreeMap<Identifier, round2::Package>> {
        let state = self.dkg_state.as_ref()
            .ok_or_else(|| SignerError::Internal("DKG state not found".into()))?;

        let round1_secret: round1::SecretPackage =
            serde_json::from_slice(state.round1_secret_package.as_ref().unwrap())
                .map_err(|e| SignerError::Internal(format!("deserialize round1 secret: {e}")))?;

        let (secret_package, packages) = frost::keys::dkg::part2(
            round1_secret,
            round1_packages,
        ).map_err(|e| SignerError::Internal(format!("DKG round2 failed: {e}")))?;

        let secret_bytes = serde_json::to_vec(&secret_package)
            .map_err(|e| SignerError::Internal(format!("serialize round2 secret: {e}")))?;

        if let Some(s) = &mut self.dkg_state {
            s.round2_secret_package = Some(secret_bytes);
        }

        Ok(packages)
    }

    async fn finalize_dkg(
        &mut self,
        round1_packages: &BTreeMap<Identifier, round1::Package>,
        round2_packages: &BTreeMap<Identifier, round2::Package>,
    ) -> Result<(KeyPackage, PublicKeyPackage)> {
        let state = self.dkg_state.as_ref()
            .ok_or_else(|| SignerError::Internal("DKG state not found".into()))?;

        let round2_secret: round2::SecretPackage =
            serde_json::from_slice(state.round2_secret_package.as_ref().unwrap())
                .map_err(|e| SignerError::Internal(format!("deserialize round2 secret: {e}")))?;

        let (key_package, pubkey_package) = frost::keys::dkg::part3(
            &round2_secret,
            round1_packages,
            round2_packages,
        ).map_err(|e| SignerError::Internal(format!("DKG finalize failed: {e}")))?;

        self.key_package = Some(key_package.clone());
        self.public_key_package = Some(pubkey_package.clone());

        Ok((key_package, pubkey_package))
    }

    async fn generate_nonce_share(&mut self) -> Result<SigningNonces> {
        let kp = self.key_package.as_ref()
            .ok_or_else(|| SignerError::Internal("KeyPackage not initialized".into()))?;

        let mut rng = OsRng;
        let (nonces, _commitments) = frost::round1::commit(kp.signing_share(), &mut rng);

        self.last_signing_nonces = Some(nonces.clone());

        Ok(nonces)
    }

    async fn create_partial_signature(
        &self,
        signing_package: &SigningPackage,
        nonces: &SigningNonces,
        key_package: &KeyPackage,
    ) -> Result<SignatureShare> {
        frost::round2::sign(signing_package, nonces, key_package)
            .map_err(|e| SignerError::Internal(format!("sign failed: {e}")))
    }

    async fn get_intermediate_values(&self, message: &[u8]) -> Result<Vec<u8>> {
        Ok(message.to_vec())
    }
}


#[cfg(test)]
mod tests {
    use std::ops::Deref;
    use super::*;
    use crate::config::SignerConfig;
    use serde::Serialize;
    use tokio::runtime::Runtime;

    fn make_config(id: u16) -> SignerConfig {
        SignerConfig {
            participant_id: Identifier::try_from(id).unwrap(),
            threshold: 2,
            total_participants: 3,
            private_key_hex: Option::from(String::new()),
        }
    }
    #[test]
    fn test_signer_config_valid() {
        let cfg = SignerConfig::new(1, 2, 3).unwrap();
        assert_eq!(cfg.threshold, 2);
        assert_eq!(cfg.total_participants, 3);
        let expected_id = Identifier::try_from(1).unwrap();
        assert_eq!(cfg.participant_id, expected_id);
    }

    #[test]
    fn test_signer_config_invalid_threshold() {
        let cfg = SignerConfig::new(1, 5, 3);
        assert!(cfg.is_err());
    }

    fn create_signer(id: u16, threshold: u32, total_participants: u32) -> FrostSigner {
        let config = SignerConfig::new(id, threshold, total_participants)
            .expect("Failed to create SignerConfig");

        FrostSigner::new(config)
    }

    #[tokio::test]
    async fn test_new_signer_defaults() {
        let config = make_config(1);
        let signer = FrostSigner::new(config.clone());
        assert_eq!(signer.threshold, 2);
        assert_eq!(signer.total_participants, 3);
        assert!(signer.key_package.is_none());
        assert_eq!(signer.get_participant_id(), &config.participant_id);
    }

    #[tokio::test]
    async fn test_set_parameters() {
        let mut signer = FrostSigner::new(make_config(1));
        signer.set_parameters(5, 10);
        assert_eq!(signer.threshold, 5);
        assert_eq!(signer.total_participants, 10);
    }

    #[tokio::test]
    async fn test_generate_keys_with_trusted_dealer() {
        let mut rng = rand::thread_rng();
        let (keys, pubpkg) =
            FrostSigner::generate_keys_with_trusted_dealer(2, &mut rng).unwrap();
        assert_eq!(keys.len(), 2);
        assert!(!pubpkg.verifying_shares().is_empty());
    }

    #[test]
    fn test_signer_config_invalid_participant_id() {
        let cfg = SignerConfig::new(0, 2, 3);
        assert!(cfg.is_err());
    }

    #[tokio::test]
    async fn test_get_public_key_share_with_key_package() {
        let mut rng = rand::thread_rng();
        let (keys, _pubpkg) =
            FrostSigner::generate_keys_with_trusted_dealer(2, &mut rng).unwrap();
        let mut signer = FrostSigner::new(make_config(1));
        signer.key_package = keys.values().next().cloned();

        let pk = signer.get_public_key_share().await.unwrap();
        assert!(pk.serialize().len() > 0);
    }

    #[test]
    fn test_get_public_key_share_before_finalize_should_fail() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let signer = FrostSigner::new(make_config(1));
            let res = signer.get_public_key_share().await;
            assert!(res.is_err());
        });
    }

    #[tokio::test]
    async fn test_intermediate_values() {
        let signer = FrostSigner::new(make_config(1));
        let msg = b"hello frost";
        let res = signer.get_intermediate_values(msg);
        assert_eq!(res.await.unwrap(), msg.as_slice());
    }

    #[tokio::test]
    async fn test_get_public_key_share_with_public_key_package() {
        let mut rng = rand::thread_rng();
        let (_keys, pubpkg) =
            FrostSigner::generate_keys_with_trusted_dealer(2, &mut rng).unwrap();
        let mut signer = FrostSigner::new(make_config(1));
        signer.public_key_package = Some(pubpkg.clone());

        let pk = signer.get_public_key_share().await.unwrap();
        assert!(pk.serialize().len() > 0);
    }

    #[tokio::test]
    async fn test_get_public_key_share_error() {
        let signer = FrostSigner::new(make_config(1));
        let res = signer.get_public_key_share().await;
        assert!(res.is_err());
    }

    fn map_without_self<T: Clone>(
        all: &BTreeMap<Identifier, T>,
        me: &Identifier,
    ) -> BTreeMap<Identifier, T> {
        all.iter()
            .filter(|(id, _)| *id != me)
            .map(|(id, pkg)| (*id, pkg.clone()))
            .collect()
    }

    #[tokio::test]
    async fn test_get_intermediate_values() {
        let signer = FrostSigner::new(make_config(1));
        let msg = b"abc123";
        let res = signer.get_intermediate_values(msg).await.unwrap();
        assert_eq!(res, msg.to_vec());
    }


}
