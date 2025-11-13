mod utils;

mod tests {
    use std::{collections::BTreeMap, str::FromStr, sync::Arc};

    use frost::{
        aggregator::FrostAggregator, mocks::MockSignerClient, traits::SignerClient, types::TweakBytes,
        utils::generate_tweak_bytes,
    };
    use frost_secp256k1_tr::Identifier;
    use gateway_config_parser::config::ServerConfig;
    use gateway_local_db_store::{
        schemas::{
            deposit_address::{DepositAddrInfo, DepositAddressStorage, InnerAddress, VerifiersResponses},
            dkg_share::DkgShareGenerate,
            user_identifier::UserIdentifierStorage,
        },
        storage::LocalDbStorage,
    };
    use global_utils::common_types::get_uuid;
    use persistent_storage::{
        config::PostgresDbCredentials,
        error::DbError,
        init::{PostgresPool, PostgresRepo},
    };
    use tracing::info;

    use crate::utils::common::{GATEWAY_CONFIG_PATH, MIGRATOR, TEST_LOGGER, create_mock_verifiers_map};

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn test_create_session(db: PostgresPool) -> eyre::Result<()> {
        let _logger_guard = &*TEST_LOGGER;

        let server_config = ServerConfig::init_config(GATEWAY_CONFIG_PATH.to_string());
        let local_repo = LocalDbStorage {
            postgres_repo: PostgresRepo { pool: db },
            network: server_config.network.network,
        };
        let shared_local_repo = Arc::new(local_repo);

        let dkg_share_id = shared_local_repo.generate_dkg_share_entity().await?;
        let verifiers_map = create_mock_verifiers_map().await;
        let aggregator = FrostAggregator::new(verifiers_map, shared_local_repo.clone(), shared_local_repo.clone());
        let _public_key_package = aggregator.run_dkg_flow(&dkg_share_id).await?;

        info!("{dkg_share_id}");
        let (rune_id, is_issuer) = ("1:18000".to_string(), true);
        let user_ids = shared_local_repo
            .get_random_unused_dkg_share(&rune_id, is_issuer)
            .await?;

        assert_eq!(
            Some(user_ids.clone()),
            shared_local_repo.get_row_by_user_id(user_ids.user_id, &rune_id).await?
        );

        let deposit_addr_info = DepositAddrInfo {
            dkg_share_id: user_ids.dkg_share_id,
            nonce: generate_tweak_bytes(),
            deposit_address: InnerAddress::BitcoinAddress(
                bitcoin::Address::from_str("bc1ph50zvqvgdexjrwn33gy2ej659uvlm02ak9xwqwg7ll7dtvjelj0srp48n8")?
                    .assume_checked(),
            ),
            bridge_address: None,
            is_btc: true,
            amount: 12344,
            confirmation_status: VerifiersResponses {
                responses: Default::default(),
            },
        };

        shared_local_repo
            .insert_deposit_addr_info(deposit_addr_info.clone())
            .await?;
        assert_eq!(
            Some(deposit_addr_info.clone()),
            shared_local_repo
                .get_row_by_deposit_address(&deposit_addr_info.deposit_address)
                .await?
        );

        Ok(())
    }
}
