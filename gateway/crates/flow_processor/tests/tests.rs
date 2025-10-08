#[cfg(test)]
mod integration_tests {
    use std::collections::BTreeMap;
    use bitcoin::secp256k1::PublicKey;
    use bitcoin::Network;
    use frost::aggregator::FrostAggregator;
    use frost::mocks::{MockAggregatorMusigIdStorage, MockAggregatorSignSessionStorage, MockSignerClient, MockSignerMusigIdStorage, MockSignerSignSessionStorage};
    use frost::types::MusigId;
    use gateway_config_parser::config::ServerConfig;
    use gateway_flow_processor::error::FlowProcessorError;
    use gateway_flow_processor::flow_sender::{FlowSender, TypedMessageSender};
    use gateway_flow_processor::init::create_flow_processor;
    use gateway_flow_processor::types::{BridgeRunesRequest, IssueBtcDepositAddressRequest, IssueSparkDepositAddressRequest};
    use gateway_local_db_store::storage::LocalDbStorage;
    use std::str::FromStr;
    use std::sync::Arc;
    use frost_secp256k1_tr::Identifier;
    use tokio;
    use frost::signer::FrostSigner;
    use frost::traits::SignerClient;

    struct TestEnvironment {
        flow_sender: FlowSender,
        storage: Arc<LocalDbStorage>,
        network: Network,
    }

    fn create_signer(identifier: u16) -> FrostSigner {
        FrostSigner::new(
            identifier,
            Arc::new(MockSignerMusigIdStorage::default()),
            Arc::new(MockSignerSignSessionStorage::default()),
            3,
            2,
        )
            .unwrap()
    }

    fn create_verifiers_map_easy() -> BTreeMap<Identifier, Arc<dyn SignerClient>> {
        let signer1 = create_signer(1);
        let signer2 = create_signer(2);
        let signer3 = create_signer(3);

        let mock_signer_client1 = MockSignerClient::new(signer1);
        let mock_signer_client2 = MockSignerClient::new(signer2);
        let mock_signer_client3 = MockSignerClient::new(signer3);

        let identifier_1: Identifier = 1.try_into().unwrap();
        let identifier_2: Identifier = 2.try_into().unwrap();
        let identifier_3: Identifier = 3.try_into().unwrap();

        BTreeMap::from([
            (identifier_1, Arc::new(mock_signer_client1) as Arc<dyn SignerClient>),
            (identifier_2, Arc::new(mock_signer_client2) as Arc<dyn SignerClient>),
            (identifier_3, Arc::new(mock_signer_client3) as Arc<dyn SignerClient>),
        ])
    }

    impl TestEnvironment {
        async fn setup() -> Result<Self, FlowProcessorError> {
            let config_path = std::env::var("TEST_CONFIG_PATH")
                .unwrap_or_else(|_| "config/test_config.toml".to_string());

            let server_config = ServerConfig::init_config(config_path);

            let network = server_config.network.network;

            let test_db_url = format!("{}_test_{}",
                                      server_config.database.url,
                                      uuid::Uuid::new_v4()
            );

            let storage = Arc::new(
                LocalDbStorage::new(test_db_url, network)
                    .await
                    .expect("Failed to create test storage")
            );

            let cancellation_retries = server_config.flow_processor.cancellation_retries;

            let frost_musig_storage = MockAggregatorMusigIdStorage::default();
            let frost_sign_storage = MockAggregatorSignSessionStorage::default();
            let verifiers_map = create_verifiers_map_easy();

            let frost_aggregator = FrostAggregator::new(
                verifiers_map,
                Arc::new(frost_musig_storage),
                Arc::new(frost_sign_storage),
            );

            let (mut flow_processor, flow_sender) = create_flow_processor(
                server_config,
                storage.clone(),
                cancellation_retries,
                frost_aggregator,
                network,
            )
                .await?;

            tokio::spawn(async move {
                flow_processor.run().await;
            });

            Ok(Self {
                flow_sender,
                storage,
                network,
            })
        }

        async fn teardown(self) {
            self.flow_sender.shutdown().await;
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

            if let Ok(conn) = self.storage.get_conn().await {
                let _ = sqlx::query("DROP SCHEMA public CASCADE; CREATE SCHEMA public;")
                    .execute(&conn)
                    .await;
            }
        }
    }

    fn create_test_musig_id(seed: u64, rune_id: &str) -> MusigId {
        let user_public_key = PublicKey::from_str(
            "02a1633cafcc01ebfb6d78e39f687a1f0995c62fc95f51ead10a02ee0be551b5dc"
        ).unwrap();

        MusigId::User {
            user_public_key,
            rune_id: format!("{}:{}", rune_id, seed),
        }
    }

    fn create_issuer_musig_id(rune_id: &str) -> MusigId {
        let issuer_public_key = PublicKey::from_str(
            "03b6f8c3d9d6e6e5f4d3c2b1a0998877665544332211ffeeddccbbaa99887766"
        ).unwrap();

        MusigId::Issuer {
            issuer_public_key,
            rune_id: rune_id.to_string(),
        }
    }

    #[tokio::test]
    async fn test_issue_btc_deposit_address_success() {
        let env = TestEnvironment::setup().await
            .expect("Failed to setup test environment");

        let musig_id = create_test_musig_id(1, "840000:3");
        let amount = 100000u64;

        let request = IssueBtcDepositAddressRequest {
            musig_id: musig_id.clone(),
            amount,
        };

        let response = env.flow_sender.send(request).await;

        assert!(
            response.is_ok(),
            "Failed to issue BTC deposit address: {:?}",
            response.err()
        );

        let addr = response.unwrap().addr_to_replenish;

        use gateway_local_db_store::schemas::deposit_address::*;

        let stored_info = env.storage
            .get_row_by_deposit_address(InnerAddress::BitcoinAddress(addr.clone()))
            .await
            .expect("Failed to query storage");

        assert!(stored_info.is_some(), "Address not found in storage");
        let stored_info = stored_info.unwrap();

        assert_eq!(stored_info.musig_id, musig_id);
        assert_eq!(stored_info.amount, amount);
        assert!(stored_info.is_btc);
        assert_eq!(stored_info.deposit_address, InnerAddress::BitcoinAddress(addr));

        env.teardown().await;
    }

    #[tokio::test]
    async fn test_issue_spark_deposit_address_success() {
        let env = TestEnvironment::setup().await
            .expect("Failed to setup test environment");

        let musig_id = create_test_musig_id(2, "840000:5");
        let amount = 50000u64;

        let request = IssueSparkDepositAddressRequest {
            musig_id: musig_id.clone(),
            amount,
        };

        let response = env.flow_sender.send(request).await;

        assert!(
            response.is_ok(),
            "Failed to issue Spark deposit address: {:?}",
            response.err()
        );

        let spark_addr = response.unwrap().addr_to_replenish;

        assert!(
            spark_addr.starts_with("spark") || spark_addr.starts_with("tspk"),
            "Invalid Spark address format: {}",
            spark_addr
        );

        use gateway_local_db_store::schemas::deposit_address::*;

        let stored_info = env.storage
            .get_row_by_deposit_address(InnerAddress::SparkAddress(spark_addr.clone()))
            .await
            .expect("Failed to query storage");

        assert!(stored_info.is_some(), "Spark address not found in storage");
        let stored_info = stored_info.unwrap();

        assert_eq!(stored_info.musig_id, musig_id);
        assert_eq!(stored_info.amount, amount);
        assert!(!stored_info.is_btc);
        assert_eq!(
            stored_info.deposit_address,
            InnerAddress::SparkAddress(spark_addr)
        );

        env.teardown().await;
    }

    #[tokio::test]
    async fn test_issue_address_existing_musig_id() {
        let env = TestEnvironment::setup().await
            .expect("Failed to setup test environment");

        let musig_id = create_test_musig_id(3, "840000:7");
        let amount = 75000u64;

        let request1 = IssueBtcDepositAddressRequest {
            musig_id: musig_id.clone(),
            amount,
        };

        let response1 = env.flow_sender.send(request1).await
            .expect("First request should succeed");

        let addr1 = response1.addr_to_replenish;

        let request2 = IssueBtcDepositAddressRequest {
            musig_id: musig_id.clone(),
            amount,
        };

        let response2 = env.flow_sender.send(request2).await
            .expect("Second request should succeed");

        let addr2 = response2.addr_to_replenish;

        assert_ne!(
            addr1, addr2,
            "Addresses should be different due to different nonces"
        );

        env.teardown().await;
    }

    #[tokio::test]
    async fn test_bridge_runes_full_flow() {
        let env = TestEnvironment::setup().await
            .expect("Failed to setup test environment");

        let rune_id = "840000:10";
        let musig_id = create_test_musig_id(4, rune_id);
        let amount = 100000u64;

        let issue_request = IssueBtcDepositAddressRequest {
            musig_id: musig_id.clone(),
            amount,
        };

        let issue_response = env.flow_sender.send(issue_request).await
            .expect("Failed to issue BTC address");

        let btc_address = issue_response.addr_to_replenish;

        let spark_musig_id = create_test_musig_id(5, rune_id);
        let spark_request = IssueSparkDepositAddressRequest {
            musig_id: spark_musig_id,
            amount,
        };

        let spark_response = env.flow_sender.send(spark_request).await
            .expect("Failed to issue Spark address");

        let spark_address = spark_response.addr_to_replenish;

        use gateway_local_db_store::schemas::deposit_address::*;

        let mut deposit_info = env.storage
            .get_row_by_deposit_address(InnerAddress::BitcoinAddress(btc_address.clone()))
            .await
            .expect("Failed to get deposit info")
            .expect("Deposit info not found");

        deposit_info.bridge_address = Some(InnerAddress::SparkAddress(spark_address.clone()));

        env.storage
            .set_deposit_addr_info(deposit_info)
            .await
            .expect("Failed to update deposit info");

        let bridge_request = BridgeRunesRequest {
            btc_address: btc_address.clone(),
        };

        let bridge_response = env.flow_sender.send(bridge_request).await;

        assert!(
            bridge_response.is_ok(),
            "Failed to bridge runes: {:?}",
            bridge_response.err()
        );

        env.teardown().await;
    }

    #[tokio::test]
    async fn test_bridge_runes_missing_bridge_address() {
        let env = TestEnvironment::setup().await
            .expect("Failed to setup test environment");

        let musig_id = create_test_musig_id(6, "840000:12");
        let amount = 100000u64;

        let issue_request = IssueBtcDepositAddressRequest {
            musig_id,
            amount,
        };

        let issue_response = env.flow_sender.send(issue_request).await
            .expect("Failed to issue BTC address");

        let btc_address = issue_response.addr_to_replenish;

        let bridge_request = BridgeRunesRequest {
            btc_address,
        };

        let bridge_response = env.flow_sender.send(bridge_request).await;

        assert!(
            bridge_response.is_err(),
            "Bridge should fail without bridge address"
        );

        match bridge_response.err().unwrap() {
            FlowProcessorError::InvalidDataError(msg) => {
                assert!(
                    msg.contains("Bridge address not found") ||
                        msg.contains("bridge_address"),
                    "Unexpected error message: {}", msg
                );
            }
            other => panic!("Expected InvalidDataError, got: {:?}", other),
        }

        env.teardown().await;
    }

    #[tokio::test]
    async fn test_concurrent_requests() {
        let env = TestEnvironment::setup().await
            .expect("Failed to setup test environment");

        let mut handles = vec![];

        for i in 0..10 {
            let sender = env.flow_sender.clone();
            let rune_id = format!("840000:{}", 20 + i);

            let handle = tokio::spawn(async move {
                let musig_id = create_test_musig_id(100 + i, &rune_id);
                let request = IssueBtcDepositAddressRequest {
                    musig_id,
                    amount: 10000 + (i * 1000) as u64,
                };
                sender.send(request).await
            });
            handles.push(handle);
        }

        let results = futures::future::join_all(handles).await;

        let mut addresses = std::collections::HashSet::new();

        for (i, result) in results.iter().enumerate() {
            assert!(result.is_ok(), "Task {} panicked", i);

            let response = result.as_ref().unwrap();
            assert!(
                response.is_ok(),
                "Request {} failed: {:?}",
                i,
                response.as_ref().err()
            );

            let addr = response.as_ref().unwrap().addr_to_replenish.clone();
            assert!(
                addresses.insert(addr.to_string()),
                "Duplicate address generated"
            );
        }

        assert_eq!(addresses.len(), 10, "Should have 10 unique addresses");

        env.teardown().await;
    }

    #[tokio::test]
    async fn test_graceful_shutdown() {
        let env = TestEnvironment::setup().await
            .expect("Failed to setup test environment");

        for i in 0..3 {
            let musig_id = create_test_musig_id(200 + i, &format!("840000:{}", 50 + i));
            let request = IssueBtcDepositAddressRequest {
                musig_id,
                amount: 10000,
            };

            let _response = env.flow_sender.send(request).await;
        }

        env.flow_sender.shutdown().await;

        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        let musig_id = create_test_musig_id(300, "840000:100");
        let request = IssueBtcDepositAddressRequest {
            musig_id,
            amount: 10000,
        };

        let response = env.flow_sender.send(request).await;

        assert!(
            response.is_err(),
            "Should fail after shutdown"
        );

        match response.err().unwrap() {
            FlowProcessorError::ChannelClosedError(_) => {

            }
            other => panic!("Expected ChannelClosedError, got: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_different_musig_id_types() {
        let env = TestEnvironment::setup().await
            .expect("Failed to setup test environment");

        let rune_id = "840000:200";

        let user_musig_id = create_test_musig_id(400, rune_id);
        let user_request = IssueBtcDepositAddressRequest {
            musig_id: user_musig_id.clone(),
            amount: 50000,
        };

        let user_response = env.flow_sender.send(user_request).await;
        assert!(user_response.is_ok(), "User MusigId should work");

        let issuer_musig_id = create_issuer_musig_id(rune_id);
        let issuer_request = IssueBtcDepositAddressRequest {
            musig_id: issuer_musig_id.clone(),
            amount: 75000,
        };

        let issuer_response = env.flow_sender.send(issuer_request).await;
        assert!(issuer_response.is_ok(), "Issuer MusigId should work");

        use gateway_local_db_store::schemas::deposit_address::*;

        let user_addr = user_response.unwrap().addr_to_replenish;
        let issuer_addr = issuer_response.unwrap().addr_to_replenish;

        let user_info = env.storage
            .get_row_by_deposit_address(InnerAddress::BitcoinAddress(user_addr))
            .await
            .expect("Failed to query user address")
            .expect("User address not found");

        let issuer_info = env.storage
            .get_row_by_deposit_address(InnerAddress::BitcoinAddress(issuer_addr))
            .await
            .expect("Failed to query issuer address")
            .expect("Issuer address not found");

        assert_eq!(user_info.musig_id.get_rune_id(), rune_id);
        assert_eq!(issuer_info.musig_id.get_rune_id(), rune_id);

        env.teardown().await;
    }
}