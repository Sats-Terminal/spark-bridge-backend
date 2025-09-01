#[cfg(test)]
mod error_tests {
    use btc_aggregator::errors::AggregatorError;

    #[test]
    fn test_aggregator_error_display() {
        let error = AggregatorError::SessionNotFound {
            id: "test-session".to_string()
        };
        let error_string = format!("{}", error);
        assert!(error_string.contains("test-session"));
    }

    #[test]
    fn test_aggregator_error_debug() {
        let error = AggregatorError::InsufficientParticipants { got: 1, need: 2 };
        let debug_string = format!("{:?}", error);
        assert!(debug_string.contains("InsufficientParticipants"));
    }
}
#[cfg(test)]
mod property_tests {
    use btc_aggregator::config::AggregatorConfig;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn test_valid_threshold_configurations(
            threshold in 1u32..10,
            extra_participants in 0u32..10
        ) {
            let total = threshold + extra_participants;
            let config_result = AggregatorConfig::new(threshold, total);
            prop_assert!(config_result.is_ok());

            let config = config_result.unwrap();
            prop_assert_eq!(config.threshold, threshold);
            prop_assert_eq!(config.total_participants, total);
        }

        #[test]
        fn test_invalid_threshold_configurations(
            threshold in 1u32..10,
            total in 1u32..10
        ) {
            prop_assume!(threshold > total);
            let config_result = AggregatorConfig::new(threshold, total);
            prop_assert!(config_result.is_err());
        }
    }
}
#[cfg(test)]
mod tests {
    use btc_aggregator::aggregator::FrostAggregator;
    use btc_aggregator::config::AggregatorConfig;
    use btc_aggregator::errors::AggregatorError;
    use btc_aggregator::session::SessionState;
    use btc_signer::api::Signer;
    use btc_signer::config::SignerConfig;
    use btc_signer::frost::FrostSigner;
    use frost_secp256k1::Identifier;
    use rand::rngs::OsRng;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::Mutex;

    fn create_test_config() -> AggregatorConfig {
        AggregatorConfig::new(2, 3).unwrap()
            .with_timeout(Duration::from_secs(30))
            .with_max_sessions(5)
    }

    fn create_test_participants() -> Vec<Identifier> {
        vec![
            Identifier::try_from(1u16).unwrap(),
            Identifier::try_from(2u16).unwrap(),
            Identifier::try_from(3u16).unwrap(),
        ]
    }

    async fn create_test_signers(count: u16, threshold: u32, total: u32) -> Vec<Arc<Mutex<FrostSigner>>> {
        let mut signers = Vec::new();

        for i in 1..=count {
            let signer_config = SignerConfig::new(i, threshold, total).unwrap();
            let mut signer = FrostSigner::new(signer_config);
            signer.set_parameters(threshold as u16, total as u16);
            signers.push(Arc::new(Mutex::new(signer)));
        }

        signers
    }

    async fn setup_aggregator_with_signers(threshold: u32, total: u32) -> (FrostAggregator, Vec<Arc<Mutex<FrostSigner>>>) {
        let config = AggregatorConfig::new(threshold, total).unwrap()
            .with_timeout(Duration::from_secs(30))
            .with_max_sessions(10);
        let aggregator = FrostAggregator::new(config).unwrap();

        let signers = create_test_signers(total as u16, threshold, total).await;

        for signer in &signers {
            aggregator.add_signer(Arc::clone(signer) as Arc<Mutex<dyn Signer + Send + Sync>>).await.unwrap();
        }

        (aggregator, signers)
    }

    #[tokio::test]
    async fn test_new_aggregator() {
        let config = create_test_config();
        let aggregator = FrostAggregator::new(config.clone()).unwrap();

        assert!(aggregator.signers.read().await.is_empty());
        assert!(aggregator.sessions.read().await.is_empty());
        assert!(aggregator.cleanup_task.is_none());
    }

    #[tokio::test]
    async fn test_new_aggregator_invalid_config() {
        let result = AggregatorConfig::new(0, 3);
        assert!(result.is_err());

        let result = AggregatorConfig::new(5, 3);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_add_signer() {
        let config = create_test_config();
        let aggregator = FrostAggregator::new(config).unwrap();

        let signer_config = SignerConfig::new(1, 2, 3).unwrap();
        let signer = Arc::new(Mutex::new(FrostSigner::new(signer_config)));

        let result = aggregator.add_signer(signer.clone() as Arc<Mutex<dyn Signer + Send + Sync>>).await;
        assert!(result.is_ok());

        let signers = aggregator.signers.read().await;
        assert_eq!(signers.len(), 1);
        assert!(signers.contains_key(&Identifier::try_from(1u16).unwrap()));
    }

    #[tokio::test]
    async fn test_add_duplicate_signer() {
        let config = create_test_config();
        let aggregator = FrostAggregator::new(config).unwrap();

        let signer_config = SignerConfig::new(1, 2, 3).unwrap();
        let signer1 = Arc::new(Mutex::new(FrostSigner::new(signer_config.clone())));
        let signer2 = Arc::new(Mutex::new(FrostSigner::new(signer_config))); // Same ID

        aggregator.add_signer(signer1 as Arc<Mutex<dyn Signer + Send + Sync>>).await.unwrap();
        let result = aggregator.add_signer(signer2 as Arc<Mutex<dyn Signer + Send + Sync>>).await;

        assert!(result.is_err());
        if let Err(AggregatorError::ParticipantExists { id }) = result {
            assert!(!id.is_empty());
        } else {
            panic!("Expected ParticipantExists error");
        }
    }

    #[tokio::test]
    async fn test_remove_signer() {
        let config = create_test_config();
        let aggregator = FrostAggregator::new(config).unwrap();

        let signer_config = SignerConfig::new(1, 2, 3).unwrap();
        let signer = Arc::new(Mutex::new(FrostSigner::new(signer_config)));
        let participant_id = Identifier::try_from(1u16).unwrap();

        aggregator.add_signer(signer as Arc<Mutex<dyn Signer + Send + Sync>>).await.unwrap();
        let result = aggregator.remove_signer(&participant_id).await;
        assert!(result.is_ok());

        let signers = aggregator.signers.read().await;
        assert!(signers.is_empty());
    }

    #[tokio::test]
    async fn test_remove_nonexistent_signer() {
        let config = create_test_config();
        let aggregator = FrostAggregator::new(config).unwrap();
        let participant_id = Identifier::try_from(1u16).unwrap();

        let result = aggregator.remove_signer(&participant_id).await;
        assert!(result.is_err());
        if let Err(AggregatorError::ParticipantNotFound { id }) = result {
            assert!(!id.is_empty());
        } else {
            panic!("Expected ParticipantNotFound error");
        }
    }

    #[tokio::test]
    async fn test_start_dkg_session_success() {
        let (aggregator, _) = setup_aggregator_with_signers(2, 3).await;
        let participants = create_test_participants();

        let result = aggregator.start_dkg_session(participants.clone()).await;
        assert!(result.is_ok());

        let session_id = result.unwrap();
        let sessions = aggregator.sessions.read().await;
        assert!(sessions.contains_key(&session_id));

        let session = sessions.get(&session_id).unwrap();
        assert_eq!(session.participants, participants);
        assert!(matches!(session.state, SessionState::DkgRound1));
    }

    #[tokio::test]
    async fn test_start_dkg_session_insufficient_participants() {
        let (aggregator, _) = setup_aggregator_with_signers(2, 3).await;
        let participants = vec![Identifier::try_from(1u16).unwrap()]; // Only 1, need 2

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
    async fn test_start_dkg_session_unknown_participant() {
        let config = create_test_config();
        let aggregator = FrostAggregator::new(config).unwrap();
        let participants = create_test_participants();
        
        let result = aggregator.start_dkg_session(participants).await;
        assert!(result.is_err());
        if let Err(AggregatorError::ParticipantNotFound { id }) = result {
            assert!(!id.is_empty());
        } else {
            panic!("Expected ParticipantNotFound error");
        }
    }

    #[tokio::test]
    async fn test_start_dkg_session_max_sessions_reached() {
        let mut config = AggregatorConfig::new(2, 3).unwrap();
        config.max_concurrent_sessions = 1;
        let aggregator = FrostAggregator::new(config).unwrap();
        let participants = create_test_participants();

        let signers = create_test_signers(3, 2, 3).await;
        for signer in &signers {
            aggregator.add_signer(Arc::clone(signer) as Arc<Mutex<dyn Signer + Send + Sync>>).await.unwrap();
        }

        aggregator.start_dkg_session(participants.clone()).await.unwrap();
        
        let result = aggregator.start_dkg_session(participants).await;
        assert!(result.is_err());
        if let Err(AggregatorError::Internal(msg)) = result {
            assert!(msg.contains("Max concurrent sessions"));
        } else {
            panic!("Expected Internal error about max sessions");
        }
    }

    #[tokio::test]
    async fn test_dkg_round1_process() {
        let (aggregator, signers) = setup_aggregator_with_signers(2, 3).await;
        let participants = create_test_participants();

        for (i, signer) in signers.iter().enumerate() {
            signer.lock().await.set_participant_id(participants[i].clone());
        }

        let session_id = aggregator.start_dkg_session(participants.clone()).await.unwrap();

        let packages = aggregator.process_dkg_round1(&session_id).await.unwrap();
        assert_eq!(packages.len(), 3);

        let sessions = aggregator.sessions.read().await;
        let session = sessions.get(&session_id).unwrap();
        assert_eq!(session.dkg_round1_packages.len(), 3);
    }

    #[tokio::test]
    async fn test_dkg_round1_invalid_state() {
        let (aggregator, _) = setup_aggregator_with_signers(2, 3).await;
        let participants = create_test_participants();
        
        let session_id = aggregator.start_dkg_session(participants).await.unwrap();
        
        {
            let mut sessions = aggregator.sessions.write().await;
            if let Some(session) = sessions.get_mut(&session_id) {
                session.state = SessionState::DkgRound2;
            }
        }
        
        let result = aggregator.process_dkg_round1(&session_id).await;
        assert!(result.is_err());
        if let Err(AggregatorError::InvalidSessionState { state }) = result {
            assert!(state.contains("DkgRound2"));
        } else {
            panic!("Expected InvalidSessionState error");
        }
    }

    #[tokio::test]
    async fn test_start_signing_session_insufficient_participants() {
        let (aggregator, _) = setup_aggregator_with_signers(2, 3).await;
        let participants = vec![Identifier::try_from(1u16).unwrap()]; 
        let message = b"test message".to_vec();
        
        let mut rng = OsRng;
        let (_, public_key_package) = FrostSigner::generate_keys_with_trusted_dealer(2, &mut rng).unwrap();

        let result = aggregator.start_signing_session(
            message,
            participants,
            public_key_package,
        ).await;

        assert!(result.is_err());
        if let Err(AggregatorError::InsufficientParticipants { got, need }) = result {
            assert_eq!(got, 1);
            assert_eq!(need, 2);
        } else {
            panic!("Expected InsufficientParticipants error");
        }
    }

    #[tokio::test]
    async fn test_cleanup_expired_sessions() {
        let mut config = AggregatorConfig::new(2, 3).unwrap();
        config.session_timeout = Duration::from_millis(100); // Very short timeout
        let aggregator = FrostAggregator::new(config).unwrap();
        let participants = create_test_participants();

        let signers = create_test_signers(3, 2, 3).await;
        for signer in &signers {
            aggregator.add_signer(Arc::clone(signer) as Arc<Mutex<dyn Signer + Send + Sync>>).await.unwrap();
        }

        let session_id = aggregator.start_dkg_session(participants).await.unwrap();

        tokio::time::sleep(Duration::from_millis(150)).await;

        aggregator.cleanup_expired_sessions().await;

        let sessions = aggregator.sessions.read().await;
        assert!(!sessions.contains_key(&session_id));
    }

    #[tokio::test]
    async fn test_start_stop_cleanup_task() {
        let config = create_test_config();
        let mut aggregator = FrostAggregator::new(config).unwrap();
        
        assert!(aggregator.cleanup_task.is_none());
        
        aggregator.start_cleanup_task().await;
        assert!(aggregator.cleanup_task.is_some());
        
        aggregator.start_cleanup_task().await;
        assert!(aggregator.cleanup_task.is_some());
        
        aggregator.stop_cleanup_task().await;
        assert!(aggregator.cleanup_task.is_none());
    }

    #[tokio::test]
    async fn test_concurrent_session_access() {
        let (aggregator, _) = setup_aggregator_with_signers(2, 10).await;
        let aggregator = Arc::new(aggregator);

        let mut handles = vec![];
        for i in 0..3 {
            let agg = Arc::clone(&aggregator);
            let participants = vec![
                Identifier::try_from((i * 3 + 1) as u16).unwrap(),
                Identifier::try_from((i * 3 + 2) as u16).unwrap(),
                Identifier::try_from((i * 3 + 3) as u16).unwrap(),
            ];
            let handle = tokio::spawn(async move {
                agg.start_dkg_session(participants).await
            });
            handles.push(handle);
        }
        
        let results: Vec<_> = futures::future::join_all(handles).await;

        for result in results {
            assert!(result.unwrap().is_ok());
        }

        let sessions = aggregator.sessions.read().await;
        assert_eq!(sessions.len(), 3);
    }

    #[tokio::test]
    async fn test_session_not_found_errors() {
        let (aggregator, _) = setup_aggregator_with_signers(2, 3).await;
        let fake_session_id = "non-existent-session";
        
        let result = aggregator.process_dkg_round1(fake_session_id).await;
        assert!(matches!(result, Err(AggregatorError::SessionNotFound { .. })));

        let result = aggregator.process_dkg_round2(fake_session_id).await;
        assert!(matches!(result, Err(AggregatorError::SessionNotFound { .. })));

        let result = aggregator.finalize_dkg(fake_session_id).await;
        assert!(matches!(result, Err(AggregatorError::SessionNotFound { .. })));

        let result = aggregator.collect_nonce_shares(fake_session_id).await;
        assert!(matches!(result, Err(AggregatorError::SessionNotFound { .. })));
    }

    #[tokio::test]
    async fn test_trusted_dealer_key_generation() {
        let threshold = 2u16;
        let mut rng = OsRng;

        let result = FrostSigner::generate_keys_with_trusted_dealer(threshold, &mut rng);
        assert!(result.is_ok());

        let (key_packages, public_key_package) = result.unwrap();
        assert_eq!(key_packages.len(), threshold as usize);
        assert!(public_key_package.verifying_shares().len() >= threshold as usize);
    }

    #[tokio::test]
    #[ignore] // run with --ignored
    async fn test_performance_many_sessions() {
        let config = AggregatorConfig::new(2, 10).unwrap()
            .with_max_sessions(100);
        let aggregator = FrostAggregator::new(config).unwrap();

        let signers = create_test_signers(10, 2, 10).await;
        for signer in &signers {
            aggregator.add_signer(Arc::clone(signer) as Arc<Mutex<dyn Signer + Send + Sync>>).await.unwrap();
        }

        let start = std::time::Instant::now();
        
        let participants: Vec<_> = (1..=5).map(|i| Identifier::try_from(i).unwrap()).collect();
        for _ in 0..50 {
            aggregator.start_dkg_session(participants.clone()).await.unwrap();
        }

        let duration = start.elapsed();
        println!("Started 50 sessions in {:?}", duration);
        
        assert!(duration < Duration::from_secs(5));
    }
}