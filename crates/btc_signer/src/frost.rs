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
pub struct DkgState {
    pub round1_secret_package: Option<Vec<u8>>,
    pub round2_secret_package: Option<Vec<u8>>,
}

pub struct FrostSigner {
    config: SignerConfig,
    pub key_package: Option<KeyPackage>,
    pub public_key_package: Option<PublicKeyPackage>,
    last_signing_nonces: Option<SigningNonces>,
    pub threshold: u16,
    pub total_participants: u16,
    pub dkg_state: Option<DkgState>,
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

    fn set_participant_id(&mut self, id: Identifier) {
        self.config.participant_id = id;
    }

    fn set_total_participants(&mut self, total: u16) {
        self.total_participants = total;
    }

    fn set_threshold(&mut self, threshold: u16) {
        self.threshold = threshold;
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

        let round1_secret_bytes = state.round1_secret_package.as_ref()
            .ok_or_else(|| SignerError::Internal("Round1 secret package not found".into()))?;

        let round1_secret: round1::SecretPackage =
            serde_json::from_slice(round1_secret_bytes)
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

        let round2_secret_bytes = state.round2_secret_package.as_ref()
            .ok_or_else(|| SignerError::Internal("Round2 secret package not found".into()))?;

        let round2_secret: round2::SecretPackage =
            serde_json::from_slice(round2_secret_bytes)
                .map_err(|e| SignerError::Internal(format!("deserialize round2 secret: {e}")))?;

        let my_id = *self.get_participant_id();
        let mut round1_packages_from_others = round1_packages.clone();
        round1_packages_from_others.remove(&my_id);

        let (key_package, pubkey_package) = frost::keys::dkg::part3(
            &round2_secret,
            &round1_packages_from_others,
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
