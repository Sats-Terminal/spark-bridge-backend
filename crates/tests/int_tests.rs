#[cfg(test)]
mod tests {
    use btc_aggregator::{AggregatorConfig, AggregatorError, FrostAggregator};
    use btc_signer::{FrostSigner, PartialSignature, ParticipantId, PublicKeyPackage, Signer, SignerConfig};
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::time::Duration;

    async fn create_test_aggregator() -> FrostAggregator {
        let config = AggregatorConfig {
            threshold: 2,
            total_participants: 3,
            session_timeout: Duration::from_secs(300),
            cleanup_interval: Duration::from_secs(60),
            max_concurrent_sessions: 10,
        };
        FrostAggregator::new(config).expect("Failed to create aggregator")
    }

    fn create_test_signer(participant_id: String) -> Arc<FrostSigner> {
        let config = SignerConfig::new(participant_id, 2, 3).expect("Failed to create config");
        let mut signer = FrostSigner::new(config);
        signer.set_parameters(2, 3);
        Arc::new(signer)
    }

    async fn setup_test_signers() -> Vec<Arc<FrostSigner>> {
        vec![
            create_test_signer("alice".to_string()),
            create_test_signer("bob".to_string()),
            create_test_signer("charlie".to_string()),
        ]
    }

    #[tokio::test]
    async fn test_signer_config_creation() {
        let config = SignerConfig::new("test_participant".to_string(), 2, 3);
        assert!(config.is_ok());

        let config = config.unwrap();
        assert_eq!(config.participant_id.as_ref(), "test_participant");
        assert_eq!(config.threshold, 2);
        assert_eq!(config.total_participants, 3);
        assert!(config.private_key_hex.is_none());

        let config_with_key = config.with_private_key("deadbeef".to_string());
        assert_eq!(config_with_key.private_key_hex, Some("deadbeef".to_string()));
    }

    #[tokio::test]
    async fn test_signer_config_validation() {
        let result = SignerConfig::new("test".to_string(), 0, 3);
        assert!(result.is_err());

        let result = SignerConfig::new("test".to_string(), 4, 3);
        assert!(result.is_err());

        let result = SignerConfig::new("test".to_string(), 3, 3);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_aggregator_creation_and_signer_management() {
        let aggregator = create_test_aggregator().await;
        let signer = create_test_signer("test_signer".to_string());

        let result = aggregator.add_signer(signer.clone()).await;
        assert!(result.is_ok());

        let result = aggregator.add_signer(signer.clone()).await;
        assert!(result.is_err());

        let participant_id = signer.get_participant_id().clone();
        let result = aggregator.remove_signer(&participant_id).await;
        assert!(result.is_ok());

        let result = aggregator.remove_signer(&participant_id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_dkg_session_creation() {
        let aggregator = create_test_aggregator().await;
        let signers = setup_test_signers().await;

        for signer in &signers {
            aggregator.add_signer(signer.clone()).await.expect("Failed to add signer");
        }

        let participants: Vec<ParticipantId> = signers
            .iter()
            .map(|s| s.get_participant_id().clone())
            .collect();

        let session_id = aggregator.start_dkg_session(participants.clone()).await;
        assert!(session_id.is_ok());

        let session_id = session_id.unwrap();
        assert!(!session_id.is_empty());

        let sessions = aggregator.list_sessions().await;
        assert!(sessions.contains(&session_id));
    }

    #[tokio::test]
    async fn test_dkg_session_insufficient_participants() {
        let aggregator = create_test_aggregator().await;
        let signer = create_test_signer("alice".to_string());

        aggregator.add_signer(signer.clone()).await.expect("Failed to add signer");

        let participants = vec![signer.get_participant_id().clone()];

        let result = aggregator.start_dkg_session(participants).await;
        assert!(result.is_err());

        if let Err(AggregatorError::InsufficientParticipants { got, need }) = result {
            assert_eq!(got, 1);
            assert_eq!(need, 2);
        } else {
            panic!("Expected InsufficientParticipants error");
        }
    }

    #[tokio::test]
    async fn test_full_dkg_cycle() {
        let aggregator = create_test_aggregator().await;
        let signers = setup_test_signers().await;

        for signer in &signers {
            aggregator.add_signer(signer.clone()).await.expect("Failed to add signer");
        }

        let participants: Vec<ParticipantId> = signers
            .iter()
            .map(|s| s.get_participant_id().clone())
            .collect();

        let session_id = aggregator
            .start_dkg_session(participants)
            .await
            .expect("Failed to start DKG session");

        let round1_packages = aggregator
            .process_dkg_round1(&session_id)
            .await
            .expect("Failed to process DKG round 1");

        assert_eq!(round1_packages.len(), 3);

        let round2_packages = aggregator
            .process_dkg_round2(&session_id, &round1_packages)
            .await
            .expect("Failed to process DKG round 2");

        assert_eq!(round2_packages.len(), 3);

        let public_key_package = aggregator
            .finalize_dkg(&session_id, &round1_packages, &round2_packages)
            .await
            .expect("Failed to finalize DKG");

        assert_eq!(public_key_package.threshold, 2);
        assert_eq!(public_key_package.total_participants, 3);
        assert_eq!(public_key_package.public_key_shares.len(), 3);
    }

    #[tokio::test]
    async fn test_signing_session_creation() {
        let aggregator = create_test_aggregator().await;
        let signers = setup_test_signers().await;

        for signer in &signers {
            aggregator.add_signer(signer.clone()).await.expect("Failed to add signer");
        }

        let participants: Vec<ParticipantId> = signers
            .iter()
            .take(2)
            .map(|s| s.get_participant_id().clone())
            .collect();

        let message = b"Hello, FROST!".to_vec();
        let public_key_package = PublicKeyPackage {
            group_public_key: secp256k1::XOnlyPublicKey::from_slice(&[1u8; 32]).unwrap(),
            public_key_shares: HashMap::new(),
            threshold: 2,
            total_participants: 3,
        };

        let session_id = aggregator
            .start_signing_session(message, participants, public_key_package)
            .await
            .expect("Failed to start signing session");

        assert!(!session_id.is_empty());

        // Проверяем, что сессия существует
        let session = aggregator.get_session(&session_id).await;
        assert!(session.is_ok());
    }

    #[tokio::test]
    async fn test_nonce_shares_collection() {
        let aggregator = create_test_aggregator().await;
        let signers = setup_test_signers().await;

        for signer in &signers {
            aggregator.add_signer(signer.clone()).await.expect("Failed to add signer");
        }

        let participants: Vec<ParticipantId> = signers
            .iter()
            .take(2)
            .map(|s| s.get_participant_id().clone())
            .collect();

        let message = b"Test message".to_vec();
        let public_key_package = PublicKeyPackage {
            group_public_key: secp256k1::XOnlyPublicKey::from_slice(&[1u8; 32]).unwrap(),
            public_key_shares: HashMap::new(),
            threshold: 2,
            total_participants: 3,
        };

        let session_id = aggregator
            .start_signing_session(message, participants.clone(), public_key_package)
            .await
            .expect("Failed to start signing session");

        let nonce_shares = aggregator
            .collect_nonce_shares(&session_id)
            .await
            .expect("Failed to collect nonce shares");

        assert_eq!(nonce_shares.len(), 2);

        for nonce_share in &nonce_shares {
            assert!(participants.contains(&nonce_share.participant_id));
        }
    }

    #[tokio::test]
    async fn test_partial_signatures_aggregation() {
        let aggregator = create_test_aggregator().await;
        let signers = setup_test_signers().await;

        for signer in &signers {
            aggregator.add_signer(signer.clone()).await.expect("Failed to add signer");
        }

        let participants: Vec<ParticipantId> = signers
            .iter()
            .take(2)
            .map(|s| s.get_participant_id().clone())
            .collect();

        let message = b"Aggregation test".to_vec();
        let public_key_package = PublicKeyPackage {
            group_public_key: secp256k1::XOnlyPublicKey::from_slice(&[1u8; 32]).unwrap(),
            public_key_shares: HashMap::new(),
            threshold: 2,
            total_participants: 3,
        };

        let session_id = aggregator
            .start_signing_session(message, participants.clone(), public_key_package)
            .await
            .expect("Failed to start signing session");

        let partial_signatures: Vec<PartialSignature> = participants
            .iter()
            .enumerate()
            .map(|(i, pid)| PartialSignature {
                participant_id: pid.clone(),
                signature_share: secp256k1::Scalar::from_be_bytes([i as u8 + 1; 32]).unwrap(),
            })
            .collect();

        let frost_signature = aggregator
            .aggregate_signatures(&session_id, &partial_signatures)
            .await
            .expect("Failed to aggregate signatures");

        assert_eq!(frost_signature.participants.len(), 2);
        assert!(frost_signature.participants.iter().all(|p| participants.contains(p)));
    }

    #[tokio::test]
    async fn test_session_management_and_statistics() {
        let aggregator = create_test_aggregator().await;
        let signers = setup_test_signers().await;

        for signer in &signers {
            aggregator.add_signer(signer.clone()).await.expect("Failed to add signer");
        }

        let participants: Vec<ParticipantId> = signers
            .iter()
            .map(|s| s.get_participant_id().clone())
            .collect();

        let session1 = aggregator
            .start_dkg_session(participants.clone())
            .await
            .expect("Failed to start DKG session 1");

        let session2 = aggregator
            .start_dkg_session(participants.clone())
            .await
            .expect("Failed to start DKG session 2");

        let sessions = aggregator.list_sessions().await;
        assert_eq!(sessions.len(), 2);
        assert!(sessions.contains(&session1));
        assert!(sessions.contains(&session2));

        let stats = aggregator.get_statistics().await;
        assert_eq!(stats.get("total_sessions"), Some(&2));
        assert_eq!(stats.get("total_signers"), Some(&3));
    }

    #[tokio::test]
    async fn test_session_cleanup() {
        let config = AggregatorConfig {
            threshold: 2,
            total_participants: 3,
            session_timeout: Duration::from_millis(100), // Очень короткий timeout для теста
            cleanup_interval: Duration::from_millis(50),
            max_concurrent_sessions: 10,
        };

        let aggregator = FrostAggregator::new(config).expect("Failed to create aggregator");
        let signers = setup_test_signers().await;

        for signer in &signers {
            aggregator.add_signer(signer.clone()).await.expect("Failed to add signer");
        }

        let participants: Vec<ParticipantId> = signers
            .iter()
            .map(|s| s.get_participant_id().clone())
            .collect();

        let session_id = aggregator
            .start_dkg_session(participants)
            .await
            .expect("Failed to start DKG session");

        tokio::time::sleep(Duration::from_millis(150)).await;

        let cleaned = aggregator
            .cleanup_expired_sessions()
            .await
            .expect("Failed to cleanup sessions");

        assert_eq!(cleaned, 1);

        let sessions = aggregator.list_sessions().await;
        assert!(!sessions.contains(&session_id));
    }

    #[tokio::test]
    async fn test_full_frost_integration() {
        let aggregator = create_test_aggregator().await;
        let signers = setup_test_signers().await;

        for signer in &signers {
            aggregator.add_signer(signer.clone()).await.expect("Failed to add signer");
        }

        let participants: Vec<ParticipantId> = signers
            .iter()
            .map(|s| s.get_participant_id().clone())
            .collect();

        let dkg_session_id = aggregator
            .start_dkg_session(participants.clone())
            .await
            .expect("Failed to start DKG session");

        let round1_packages = aggregator
            .process_dkg_round1(&dkg_session_id)
            .await
            .expect("Failed to process DKG round 1");

        let round2_packages = aggregator
            .process_dkg_round2(&dkg_session_id, &round1_packages)
            .await
            .expect("Failed to process DKG round 2");

        let public_key_package = aggregator
            .finalize_dkg(&dkg_session_id, &round1_packages, &round2_packages)
            .await
            .expect("Failed to finalize DKG");

        let message = b"Integration test message".to_vec();
        let signing_participants = participants.clone().into_iter().take(2).collect(); // Используем пороговое количество

        let signing_session_id = aggregator
            .start_signing_session(message, signing_participants, public_key_package)
            .await
            .expect("Failed to start signing session");

        let nonce_shares = aggregator
            .collect_nonce_shares(&signing_session_id)
            .await
            .expect("Failed to collect nonce shares");

        let partial_signatures = aggregator
            .collect_partial_signatures(&signing_session_id)
            .await
            .expect("Failed to collect partial signatures");

        let frost_signature = aggregator
            .aggregate_signatures(&signing_session_id, &partial_signatures)
            .await
            .expect("Failed to aggregate signatures");

        assert_eq!(nonce_shares.len(), 2);
        assert_eq!(partial_signatures.len(), 2);
        assert_eq!(frost_signature.participants.len(), 2);

        let stats = aggregator.get_statistics().await;
        assert_eq!(stats.get("total_sessions"), Some(&2));
        assert_eq!(stats.get("total_signers"), Some(&3));
    }

    #[tokio::test]
    async fn test_frost_signer_key_generation() {
        let participants = vec![
            ParticipantId("alice".to_string()),
            ParticipantId("bob".to_string()),
            ParticipantId("charlie".to_string()),
        ];

        let mut rng = rand::rngs::OsRng;

        let key_shares = FrostSigner::generate_keys_with_trusted_dealer(
            &participants,
            2,
            &mut rng,
        ).expect("Failed to generate keys with trusted dealer");

        assert_eq!(key_shares.len(), 3);

        for (participant_id, key_share) in &key_shares {
            assert_eq!(key_share.participant_id, *participant_id);
            assert_eq!(key_share.threshold, 2);
            assert_eq!(key_share.total_participants, 3);
        }
    }
}