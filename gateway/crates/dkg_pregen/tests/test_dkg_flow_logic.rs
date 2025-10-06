mod utils;
mod tests {
    use crate::utils::common::{CONFIG_PATH, TEST_LOGGER};
    use frost::aggregator::FrostAggregator;
    use frost::mocks::{MockSignerClient, MockSignerDkgShareIdStorage, MockSignerSignSessionStorage};
    use frost::signer::FrostSigner;
    use frost::traits::SignerClient;
    use frost_secp256k1_tr::Identifier;
    use gateway_config_parser::config::ServerConfig;
    use gateway_dkg_pregen::dkg_pregen_thread::DkgPregenThread;
    use gateway_local_db_store::schemas::dkg_share::DkgShareGenerate;
    use gateway_local_db_store::schemas::user_identifier::UserIdentifierData;
    use gateway_local_db_store::storage::LocalDbStorage;
    use persistent_storage::init::{PostgresPool, PostgresRepo};
    use std::collections::BTreeMap;
    use std::sync::Arc;
    use std::time::Duration;

    #[sqlx::test(migrator = "crate::utils::common::MIGRATOR")]
    async fn test_dkg_flow_logic(db: PostgresPool) -> eyre::Result<()> {
        let _logger_guard = &*TEST_LOGGER;
        _test_dkg_flow_logic(db).await
    }

    #[sqlx::test(migrator = "crate::utils::common::MIGRATOR")]
    async fn test_dkg_flow_thread(db: PostgresPool) -> eyre::Result<()> {
        let _logger_guard = &*TEST_LOGGER;
        _test_dkg_flow_logic(db).await
    }

    async fn _test_dkg_flow_thread(db: sqlx::PgPool) -> eyre::Result<()> {
        let mut server_config = ServerConfig::init_config(CONFIG_PATH.to_string());
        let local_repo = LocalDbStorage {
            postgres_repo: PostgresRepo { pool: db },
            network: server_config.network.network,
        }
        .into_shared();
        server_config.dkg_pregen_config.min_threshold = 15;
        let verifiers_map = create_verifiers_map().await;
        let aggregator = FrostAggregator::new(verifiers_map, local_repo.clone(), local_repo.clone()).into_shared();

        let _pregen_thread =
            DkgPregenThread::start(local_repo.clone(), server_config.dkg_pregen_config, aggregator).await;

        // Wait for filling dkg pregens
        tokio::time::sleep(Duration::from_millis(
            server_config.dkg_pregen_config.update_interval_millis * 3,
        ))
        .await;
        assert_eq!(
            local_repo.count_unused_dkg_shares().await?,
            server_config.dkg_pregen_config.min_threshold
        );
        assert_eq!(
            local_repo.count_unused_finalized_dkg_shares().await?,
            server_config.dkg_pregen_config.min_threshold
        );

        // Use 1 dkg share and wait for filling it
        use_one_dkg_share(&local_repo).await?;
        tokio::time::sleep(Duration::from_millis(
            server_config.dkg_pregen_config.update_interval_millis,
        ))
        .await;
        assert_eq!(
            local_repo.count_unused_dkg_shares().await?,
            server_config.dkg_pregen_config.min_threshold
        );
        assert_eq!(
            local_repo.count_unused_finalized_dkg_shares().await?,
            server_config.dkg_pregen_config.min_threshold
        );

        // Use 10 dkg shares and wait for filling it
        for _ in 0..10 {
            use_one_dkg_share(&local_repo).await?;
        }
        tokio::time::sleep(Duration::from_millis(
            server_config.dkg_pregen_config.update_interval_millis * 2,
        ))
        .await;
        assert_eq!(
            local_repo.count_unused_dkg_shares().await?,
            server_config.dkg_pregen_config.min_threshold
        );
        assert_eq!(
            local_repo.count_unused_finalized_dkg_shares().await?,
            server_config.dkg_pregen_config.min_threshold
        );

        Ok(())
    }

    async fn _test_dkg_flow_logic(db: sqlx::PgPool) -> eyre::Result<()> {
        let server_config = ServerConfig::init_config(CONFIG_PATH.to_string());
        let local_repo = Arc::new(LocalDbStorage {
            postgres_repo: PostgresRepo { pool: db },
            network: server_config.network.network,
        });
        let verifiers_map = create_verifiers_map().await;
        let aggregator = Arc::new(FrostAggregator::new(
            verifiers_map,
            local_repo.clone(),
            local_repo.clone(),
        ));
        let (unfinished_dkgs_num, finished_used_dkgs_num) = (10, 10);

        DkgPregenThread::perform_update(local_repo.clone(), aggregator.clone(), &server_config.dkg_pregen_config).await;

        assert_eq!(
            local_repo.count_unused_dkg_shares().await?,
            server_config.dkg_pregen_config.min_threshold
        );
        assert_eq!(
            local_repo.count_unused_finalized_dkg_shares().await?,
            server_config.dkg_pregen_config.min_threshold
        );

        // emulate unfinished dkgs
        for _ in 0..unfinished_dkgs_num {
            let _ = local_repo.generate_dkg_share_entity().await?;
        }
        // emulate using of dkgs
        for _ in 0..finished_used_dkgs_num {
            use_one_dkg_share(&local_repo).await?;
        }
        assert_eq!(
            local_repo.count_unused_dkg_shares().await?,
            server_config.dkg_pregen_config.min_threshold
        );
        let unused_finalized_dkg_shares = server_config.dkg_pregen_config.min_threshold - finished_used_dkgs_num;
        assert_eq!(
            local_repo.count_unused_finalized_dkg_shares().await?,
            unused_finalized_dkg_shares,
        );

        DkgPregenThread::perform_update(local_repo.clone(), aggregator.clone(), &server_config.dkg_pregen_config).await;
        assert_eq!(
            local_repo.count_unused_dkg_shares().await?,
            server_config.dkg_pregen_config.min_threshold
                + (server_config.dkg_pregen_config.min_threshold - unused_finalized_dkg_shares)
        );
        assert_eq!(
            local_repo.count_unused_finalized_dkg_shares().await?,
            server_config.dkg_pregen_config.min_threshold
        );

        Ok(())
    }

    async fn use_one_dkg_share(local_repo: &Arc<LocalDbStorage>) -> eyre::Result<()> {
        let _ = local_repo
            .get_random_unused_dkg_share(UserIdentifierData {
                public_key: "some pubkey".to_string(),
                rune_id: "1:124000".to_string(),
                is_issuer: false,
            })
            .await?;
        Ok(())
    }

    async fn create_signer(identifier: u16) -> FrostSigner {
        FrostSigner::new(
            identifier,
            Arc::new(MockSignerDkgShareIdStorage::default()),
            Arc::new(MockSignerSignSessionStorage::default()),
            3,
            2,
        )
        .unwrap()
    }

    async fn create_verifiers_map() -> BTreeMap<Identifier, Arc<dyn SignerClient>> {
        let signer1 = create_signer(1).await;
        let signer2 = create_signer(2).await;
        let signer3 = create_signer(3).await;

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
}
