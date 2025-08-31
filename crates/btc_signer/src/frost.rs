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


#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SignerConfig;
    use serde::Serialize;
    use tokio::runtime::Runtime;

    fn create_test_signer(participant_id: u16) -> FrostSigner {
        let config = make_config(participant_id);
        let mut signer = FrostSigner::new(config);
        signer.set_parameters(2, 3); // threshold=2, total=3
        signer
    }

    fn make_config(id: u16) -> SignerConfig {
        SignerConfig {
            participant_id: Identifier::try_from(id).unwrap(),
            threshold: 2,
            total_participants: 3,
            private_key_hex: Option::from(String::new()),
        }
    }

    #[tokio::test]
    async fn test_dkg_round_1_basic() {
        let mut signer = create_test_signer(1);

        let result = signer.dkg_round_1().await;
        assert!(result.is_ok(), "DKG round 1 should succeed");

        let _package = result.unwrap();

        assert!(signer.dkg_state.is_some(), "DKG state should be initialized");

        let state = signer.dkg_state.as_ref().unwrap();
        assert!(state.round1_secret_package.is_some(), "Round1 secret should be stored");
        assert!(state.round2_secret_package.is_none(), "Round2 secret should not exist yet");
    }

    #[tokio::test]
    async fn test_dkg_round_1_multiple_participants() {
        let participant_ids = [1u16, 2u16, 3u16];
        let mut signers: Vec<_> = participant_ids.iter()
            .map(|&id| create_test_signer(id))
            .collect();

        let mut packages = Vec::new();
        let mut participant_identifiers = Vec::new();

        for (i, signer) in signers.iter_mut().enumerate() {
            let package = signer.dkg_round_1().await
                .expect("DKG round 1 should succeed for all participants");
            packages.push(package);
            participant_identifiers.push(participant_ids[i]);
        }

        for i in 0..participant_identifiers.len() {
            for j in i+1..participant_identifiers.len() {
                assert_ne!(
                    participant_identifiers[i],
                    participant_identifiers[j],
                    "Each participant should have unique identifier"
                );
            }
        }

        for signer in &signers {
            assert!(signer.dkg_state.is_some(), "All participants should have DKG state");
        }
    }

    #[tokio::test]
    async fn test_dkg_round_2_with_insufficient_packages() {
        let mut signer = create_test_signer(1);

        // Выполняем round1
        signer.dkg_round_1().await.unwrap();

        let round1_packages = BTreeMap::new();
        let result = signer.dkg_round_2(&round1_packages).await;

        assert!(result.is_err(), "DKG round 2 should fail with insufficient packages");
    }

    #[tokio::test]
    async fn test_serde_support_check() {
        let mut signer = create_test_signer(1);

        let _package = signer.dkg_round_1().await.expect("Round 1 should work");

        if let Some(state) = &signer.dkg_state {
            if let Some(ref bytes) = state.round1_secret_package {
                let deser_result: serde_json::Result<serde_json::Value> = serde_json::from_slice(bytes);
                match deser_result {
                    Ok(_) => println!("Serde deserialization works"),
                    Err(e) => {
                        println!("Serde deserialization failed: {}", e);
                        println!("Bytes length: {}", bytes.len());
                        println!("First 10 bytes: {:?}", &bytes[..std::cmp::min(10, bytes.len())]);
                    }
                }
            }
        }
    }

    #[tokio::test]
    async fn test_dkg_round_2_success() {
        let participant_count = 3;
        let participant_ids: Vec<u16> = (1..=participant_count).collect();
        let mut signers: Vec<_> = participant_ids.iter()
            .map(|&id| create_test_signer(id))
            .collect();

        let mut round1_packages = BTreeMap::new();
        for (i, signer) in signers.iter_mut().enumerate() {
            let package = signer.dkg_round_1().await.unwrap();
            let participant_id = Identifier::try_from(participant_ids[i]).unwrap();
            round1_packages.insert(participant_id, package);
        }

        let participant_1_id = Identifier::try_from(1u16).unwrap();
        let mut round1_packages_for_p1 = round1_packages.clone();
        round1_packages_for_p1.remove(&participant_1_id);

        let result = signers[0].dkg_round_2(&round1_packages_for_p1).await;

        if let Err(ref e) = result {
            eprintln!("DKG round 2 failed with error: {:?}", e);
            panic!("DKG round 2 should succeed, but got error: {:?}", e);
        }

        let round2_packages = result.unwrap();

        assert!(!round2_packages.is_empty(), "Should create round2 packages");

        let state = signers[0].dkg_state.as_ref().unwrap();
        assert!(state.round1_secret_package.is_some(), "Round1 secret should still exist");
        assert!(state.round2_secret_package.is_some(), "Round2 secret should be created");

        let sender_id = Identifier::try_from(1u16).unwrap();
        assert!(!round2_packages.contains_key(&sender_id), "Should not create package for self");
    }

    #[tokio::test]
    async fn test_dkg_round_2_debug() {
        let mut signers = vec![
            create_test_signer(1),
            create_test_signer(2),
            create_test_signer(3),
        ];

        println!("Created signers with IDs: 1, 2, 3");

        let mut round1_packages = BTreeMap::new();
        for (i, signer) in signers.iter_mut().enumerate() {
            println!("Executing DKG round 1 for participant {}", i + 1);
            let package = signer.dkg_round_1().await.expect("Round 1 should work");
            let id = Identifier::try_from((i + 1) as u16).unwrap();
            round1_packages.insert(id, package);
            println!("Round 1 completed for participant {}", i + 1);
        }

        println!("Round 1 packages collected: {}", round1_packages.len());

        let participant_1_id = Identifier::try_from(1u16).unwrap();
        let mut round1_packages_for_p1 = round1_packages.clone();
        round1_packages_for_p1.remove(&participant_1_id);

        println!("Round 1 packages for participant 1 (excluding own): {}", round1_packages_for_p1.len());
        for id in round1_packages_for_p1.keys() {
            println!("Package from participant: {:?}", id);
        }

        println!("Attempting Round 2 for participant 1");
        match signers[0].dkg_round_2(&round1_packages_for_p1).await {
            Ok(packages) => {
                println!("Round 2 succeeded! Generated {} packages", packages.len());
                for (id, _) in &packages {
                    println!("Package created for participant: {:?}", id);
                }
            }
            Err(e) => {
                println!("Round 2 failed with error: {:?}", e);

                if let Some(state) = &signers[0].dkg_state {
                    println!("DKG state exists");
                    println!("Round1 secret exists: {}", state.round1_secret_package.is_some());
                    if let Some(ref bytes) = state.round1_secret_package {
                        println!("Round1 secret size: {} bytes", bytes.len());
                    }
                } else {
                    println!("No DKG state found!");
                }

                panic!("Round 2 failed: {:?}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_dkg_round_2_without_round_1() {
        let mut signer = create_test_signer(1);
        let round1_packages = BTreeMap::new();

        let result = signer.dkg_round_2(&round1_packages).await;
        assert!(result.is_err(), "DKG round 2 should fail without round 1");

        match result {
            Err(SignerError::Internal(msg)) => {
                assert!(msg.contains("DKG state not found"),
                        "Error should mention missing DKG state");
            }
            _ => panic!("Expected Internal error with DKG state message"),
        }
    }

    #[tokio::test]
    async fn test_finalize_dkg_without_previous_rounds() {
        let mut signer = create_test_signer(1);
        let round1_packages = BTreeMap::new();
        let round2_packages = BTreeMap::new();

        let result = signer.finalize_dkg(&round1_packages, &round2_packages).await;
        assert!(result.is_err(), "Finalization should fail without previous rounds");

        match result {
            Err(SignerError::Internal(msg)) => {
                assert!(msg.contains("DKG state not found"));
            }
            _ => panic!("Expected Internal error about missing DKG state"),
        }
    }

    #[tokio::test]
    async fn test_finalize_dkg_without_round_2() {
        let mut signer = create_test_signer(1);

        signer.dkg_round_1().await.unwrap();

        let round1_packages = BTreeMap::new();
        let round2_packages = BTreeMap::new();

        let result = signer.finalize_dkg(&round1_packages, &round2_packages).await;
        assert!(result.is_err(), "Finalization should fail without round 2");

        match result {
            Err(SignerError::Internal(msg)) => {
                assert!(msg.contains("Round2 secret package not found"));
            }
            _ => panic!("Expected Internal error about missing round2 secret"),
        }
    }

    #[tokio::test]
    async fn test_dkg_state_isolation() {
        let mut signer1 = create_test_signer(1);
        let mut signer2 = create_test_signer(2);

        signer1.dkg_round_1().await.unwrap();
        signer2.dkg_round_1().await.unwrap();

        let state1 = signer1.dkg_state.as_ref().unwrap();
        let state2 = signer2.dkg_state.as_ref().unwrap();

        assert_ne!(
            state1.round1_secret_package,
            state2.round1_secret_package,
            "Each participant should have unique DKG state"
        );
    }

    #[tokio::test]
    async fn test_error_handling_serialization() {
        let mut signer = create_test_signer(1);

        signer.dkg_state = Some(DkgState {
            round1_secret_package: Some(vec![0, 1, 2, 3]),
            round2_secret_package: None,
        });

        let round1_packages = BTreeMap::new();
        let result = signer.dkg_round_2(&round1_packages).await;

        assert!(result.is_err(), "Should fail with invalid serialized data");
        match result {
            Err(SignerError::Internal(msg)) => {
                assert!(msg.contains("deserialize"), "Error should mention deserialization");
            }
            _ => panic!("Expected deserialization error"),
        }
    }

    #[tokio::test]
    async fn test_participant_id_consistency() {
        let participant_id = 42u16;
        let signer = create_test_signer(participant_id);

        let expected_id = Identifier::try_from(participant_id).unwrap();
        assert_eq!(signer.get_participant_id(), &expected_id);
    }

    #[tokio::test]
    async fn test_threshold_parameters() {
        let mut signer = create_test_signer(1);

        let threshold = 3u16;
        let total_participants = 5u16;

        signer.set_parameters(threshold, total_participants);
        assert_eq!(signer.threshold, threshold);
        assert_eq!(signer.total_participants, total_participants);

        let result = signer.dkg_round_1().await;
        assert!(result.is_ok(), "DKG should work with custom parameters");
    }

    #[tokio::test]
    async fn test_finalize_dkg_complete_workflow() {
        let participant_count = 3u16;
        let participant_ids: Vec<u16> = (1..=participant_count).collect();
        let mut signers: Vec<_> = participant_ids.iter()
            .map(|&id| create_test_signer(id))
            .collect();

        let mut round1_packages = BTreeMap::new();
        for (i, signer) in signers.iter_mut().enumerate() {
            let package = signer.dkg_round_1().await.unwrap();
            let participant_id = Identifier::try_from(participant_ids[i]).unwrap();
            round1_packages.insert(participant_id, package);
        }

        let mut all_round2_packages_by_sender: BTreeMap<Identifier, BTreeMap<Identifier, round2::Package>> = BTreeMap::new();

        for (i, signer) in signers.iter_mut().enumerate() {
            let current_participant_id = Identifier::try_from(participant_ids[i]).unwrap();

            let mut packages_for_current = round1_packages.clone();
            packages_for_current.remove(&current_participant_id);

            let packages = signer.dkg_round_2(&packages_for_current).await.unwrap();
            all_round2_packages_by_sender.insert(current_participant_id, packages);
        }

        for (i, signer) in signers.iter_mut().enumerate() {
            let current_participant_id = Identifier::try_from(participant_ids[i]).unwrap();

            let mut round2_packages_for_current = BTreeMap::new();

            for (sender_id, packages_from_sender) in &all_round2_packages_by_sender {
                if let Some(package_for_current) = packages_from_sender.get(&current_participant_id) {
                    round2_packages_for_current.insert(*sender_id, package_for_current.clone());
                }
            }

            println!("Participant {:?}: received {} round2 packages",
                     current_participant_id, round2_packages_for_current.len());

            let result = signer.finalize_dkg(&round1_packages, &round2_packages_for_current).await;
            assert!(result.is_ok(), "DKG finalization should succeed for participant {:?}", current_participant_id);

            let (key_package, public_key_package) = result.unwrap();

            assert!(signer.key_package.is_some(), "Key package should be saved");
            assert!(signer.public_key_package.is_some(), "Public key package should be saved");

            assert_eq!(&key_package, signer.key_package.as_ref().unwrap());
            assert_eq!(&public_key_package, signer.public_key_package.as_ref().unwrap());

            let verifying_shares = public_key_package.verifying_shares();
            assert_eq!(verifying_shares.len(), participant_count as usize);

            for &participant_id in &participant_ids {
                let id = Identifier::try_from(participant_id).unwrap();
                assert!(
                    verifying_shares.contains_key(&id),
                    "Should have verifying share for participant {}", participant_id
                );
            }
        }
    }

    #[tokio::test]
    async fn test_complete_dkg_workflow_all_participants() {
        let participant_count = 3u16;
        let threshold = 2u16;
        let participant_ids: Vec<u16> = (1..=participant_count).collect();

        let mut signers: Vec<_> = participant_ids.iter().map(|&id| {
            let mut signer = create_test_signer(id);
            signer.set_parameters(threshold, participant_count);
            signer
        }).collect();

        let mut round1_packages = BTreeMap::new();
        for (i, signer) in signers.iter_mut().enumerate() {
            let package = signer.dkg_round_1().await
                .expect("Round 1 should succeed for all participants");
            let participant_id = Identifier::try_from(participant_ids[i]).unwrap();
            round1_packages.insert(participant_id, package);
        }

        let mut all_round2_packages_by_sender: BTreeMap<Identifier, BTreeMap<Identifier, round2::Package>> = BTreeMap::new();

        for (i, signer) in signers.iter_mut().enumerate() {
            let current_participant_id = Identifier::try_from(participant_ids[i]).unwrap();

            let mut packages_for_current = round1_packages.clone();
            packages_for_current.remove(&current_participant_id);

            let packages = signer.dkg_round_2(&packages_for_current).await
                .expect("Round 2 should succeed for all participants");

            all_round2_packages_by_sender.insert(current_participant_id, packages);
        }

        let mut results = Vec::new();
        for (i, signer) in signers.iter_mut().enumerate() {
            let current_participant_id = Identifier::try_from(participant_ids[i]).unwrap();

            let mut round2_packages_for_current = BTreeMap::new();

            for (sender_id, packages_from_sender) in &all_round2_packages_by_sender {
                if let Some(package_for_current) = packages_from_sender.get(&current_participant_id) {
                    round2_packages_for_current.insert(*sender_id, package_for_current.clone());
                }
            }

            let result = signer.finalize_dkg(&round1_packages, &round2_packages_for_current).await
                .expect(&format!("Finalization should succeed for participant {}", i + 1));
            results.push(result);
        }

        let first_group_pubkey = &results[0].1;
        for (i, (_, pubkey_package)) in results.iter().enumerate() {
            assert_eq!(
                first_group_pubkey,
                pubkey_package,
                "Participant {} should have same group public key", i + 1
            );

            assert_eq!(
                first_group_pubkey.verifying_shares(),
                pubkey_package.verifying_shares(),
                "Participant {} should have same verifying shares", i + 1
            );
        }

        for (i, signer) in signers.iter().enumerate() {
            assert!(signer.key_package.is_some(), "Participant {} should have key package", i + 1);
            assert!(signer.public_key_package.is_some(), "Participant {} should have public key package", i + 1);
        }

        for (i, (_, pubkey_package)) in results.iter().enumerate() {
            let verifying_shares = pubkey_package.verifying_shares();
            assert_eq!(verifying_shares.len(), participant_count as usize,
                       "Participant {} should have shares for all participants", i + 1);

            for &participant_id in &participant_ids {
                let id = Identifier::try_from(participant_id).unwrap();
                assert!(verifying_shares.contains_key(&id),
                        "Participant {} should have verifying share for participant {}", i + 1, participant_id);
            }
        }

        println!("All participants successfully completed DKG workflow");
        println!("All group public keys are consistent");
        println!("All verifying shares are present and consistent");
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

    #[tokio::test]
    async fn test_get_intermediate_values() {
        let signer = FrostSigner::new(make_config(1));
        let msg = b"abc123";
        let res = signer.get_intermediate_values(msg).await.unwrap();
        assert_eq!(res, msg.to_vec());
    }
}
