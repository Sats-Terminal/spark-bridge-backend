use frost::traits::SignerClient;

mod utils;
mod tests {
    use crate::utils::common::{
        GATEWAY_CONFIG_PATH, MIGRATOR, TEST_LOGGER, create_mock_signer, create_mock_verifiers_map,
    };
    use bitcoin::Network;
    use bitcoin::key::Secp256k1;
    use bitcoin::secp256k1::SecretKey;
    use frost::aggregator::FrostAggregator;
    use frost::mocks::{MockSignerClient, MockSignerDkgShareIdStorage, MockSignerSignSessionStorage};
    use frost::signer::FrostSigner;
    use frost::traits::SignerClient;
    use frost::types::{SigningMetadata, TweakBytes};
    use frost::utils::generate_tweak_bytes;
    use frost_secp256k1_tr::Identifier;
    use frost_secp256k1_tr::keys::Tweak;
    use gateway_config_parser::config::{ServerConfig, VerifierConfig};
    use gateway_local_db_store::schemas::dkg_share::{DkgShareGenerate, DkgShareGenerateError};
    use gateway_local_db_store::schemas::user_identifier::{UserIdentifierStorage, UserIds};
    use gateway_local_db_store::storage::LocalDbStorage;
    use global_utils::common_types::get_uuid;
    use global_utils::config_path::ConfigPath;
    use global_utils::logger::{LoggerGuard, init_logger};
    use persistent_storage::config::PostgresDbCredentials;
    use persistent_storage::error::DbError;
    use persistent_storage::init::{PostgresPool, PostgresRepo};
    use serde_json::json;
    use std::collections::BTreeMap;
    use std::sync::{Arc, LazyLock};
    use tracing::info;

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn test_create_session(db: PostgresPool) -> Result<(), DbError> {
        let _logger_guard = &*TEST_LOGGER;

        let server_config = ServerConfig::init_config(GATEWAY_CONFIG_PATH.to_string());
        let local_repo = LocalDbStorage {
            postgres_repo: PostgresRepo { pool: db },
            network: server_config.network.network,
        };
        let shared_local_repo = Arc::new(local_repo);

        let dkg_share_id = shared_local_repo.generate_dkg_share_entity().await?;
        info!("{dkg_share_id}");

        // Create Frost Aggregator
        let mut verifiers_map = BTreeMap::<Identifier, Arc<dyn SignerClient>>::new();
        for verifier in server_config.clone().verifiers.0 {
            let identifier: Identifier = verifier.id.try_into().unwrap();
            let verifier_client = MockSignerClient::new(create_mock_signer(verifier.id).await);
            verifiers_map.insert(identifier, Arc::new(verifier_client));
        }
        let frost_aggregator =
            FrostAggregator::new(verifiers_map, shared_local_repo.clone(), shared_local_repo.clone());

        info!(
            "pubkey package: {:?}",
            frost_aggregator.run_dkg_flow(&dkg_share_id).await
        );
        Ok(())
    }

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn test_aggregator_signer_integration_without_tap_tweak(db: PostgresPool) -> eyre::Result<()> {
        let _logger_guard = &*TEST_LOGGER;

        let tweak = None;
        _test_aggregator_signer_integration(db, tweak, false).await
    }

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn test_aggregator_signer_integration_tweaked_without_tap_tweak(db: PostgresPool) -> eyre::Result<()> {
        let _logger_guard = &*TEST_LOGGER;
        let tweak = Some(generate_tweak_bytes());
        _test_aggregator_signer_integration(db, tweak, false).await
    }

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn test_dkg_share_flow_creation(db: PostgresPool) -> eyre::Result<()> {
        let _logger_guard = &*TEST_LOGGER;
        _test_dkg_pregen_draft_flow(db).await
    }

    async fn _test_dkg_pregen_draft_flow(db: sqlx::PgPool) -> eyre::Result<()> {
        let server_config = ServerConfig::init_config(GATEWAY_CONFIG_PATH.to_string());
        let local_repo = Arc::new(LocalDbStorage {
            postgres_repo: PostgresRepo { pool: db },
            network: server_config.network.network,
        });

        let dkg_share_id = local_repo.generate_dkg_share_entity().await?;

        let verifiers_map = create_mock_verifiers_map().await;
        let aggregator = FrostAggregator::new(verifiers_map, local_repo.clone(), local_repo.clone());

        let _public_key_package = aggregator.run_dkg_flow(&dkg_share_id).await?;

        let (rune_id, is_issuer) = ("1:18000".to_string(), false);
        assert_eq!(local_repo.count_unused_dkg_shares().await?, 1);
        let user_ids = local_repo.get_random_unused_dkg_share(&rune_id, is_issuer).await?;
        assert_eq!(
            Some(UserIds {
                user_id: user_ids.user_id.clone(),
                dkg_share_id,
                rune_id: rune_id.clone(),
                is_issuer,
            }),
            local_repo.get_row_by_user_id(&*user_ids.user_id, &rune_id).await?
        );

        assert_eq!(local_repo.count_unused_dkg_shares().await?, 0);
        let obtained_value = local_repo.get_random_unused_dkg_share(&rune_id, false).await;
        assert!(matches!(
            Err::<UserIds, DkgShareGenerateError>(DkgShareGenerateError::DkgPregenFailed),
            obtained_value
        ));

        Ok(())
    }

    async fn _test_aggregator_signer_integration(
        db: sqlx::PgPool,
        tweak: Option<TweakBytes>,
        tap_tweak: bool,
    ) -> eyre::Result<()> {
        let server_config = ServerConfig::init_config(GATEWAY_CONFIG_PATH.to_string());
        let local_repo = Arc::new(LocalDbStorage {
            postgres_repo: PostgresRepo { pool: db },
            network: server_config.network.network,
        });

        let user_id = local_repo.generate_dkg_share_entity().await?;

        let verifiers_map = create_mock_verifiers_map().await;
        let aggregator = FrostAggregator::new(verifiers_map, local_repo.clone(), local_repo);

        let message_hash = b"test_message";

        let public_key_package = aggregator.run_dkg_flow(&user_id).await?;

        let metadata = SigningMetadata::Authorization;

        let signature = aggregator
            .run_signing_flow(user_id, message_hash, metadata, tweak, tap_tweak)
            .await?;

        let tweaked_public_key_package = match tweak.clone() {
            Some(tweak) => public_key_package.clone().tweak(Some(tweak.to_vec())),
            None => public_key_package.clone(),
        };
        tweaked_public_key_package
            .verifying_key()
            .verify(message_hash, &signature)?;

        Ok(())
    }
}
