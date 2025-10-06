mod utils;

mod tests {
    use crate::utils::common::{GATEWAY_CONFIG_PATH, MIGRATOR, TEST_LOGGER};
    use frost::aggregator::FrostAggregator;
    use frost::mocks::MockSignerClient;
    use frost::traits::SignerClient;
    use frost::types::TweakBytes;
    use frost::utils::generate_tweak_bytes;
    use frost_secp256k1_tr::Identifier;
    use gateway_config_parser::config::ServerConfig;
    use gateway_local_db_store::schemas::deposit_address::{
        DepositAddrInfo, DepositAddressStorage, InnerAddress, VerifiersResponses,
    };
    use gateway_local_db_store::schemas::dkg_share::DkgShareGenerate;
    use gateway_local_db_store::schemas::user_identifier::{
        UserIdentifier, UserIdentifierData, UserIdentifierStorage, UserUniqueId,
    };
    use gateway_local_db_store::storage::LocalDbStorage;
    use global_utils::common_types::get_uuid;
    use persistent_storage::config::PostgresDbCredentials;
    use persistent_storage::error::DbError;
    use persistent_storage::init::{PostgresPool, PostgresRepo};
    use std::collections::BTreeMap;
    use std::str::FromStr;
    use std::sync::Arc;
    use tracing::info;

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
        info!("{dkg_share_id}");

        let user_identifier = UserIdentifier {
            user_uuid: get_uuid(),
            dkg_share_id,
            public_key: "02D3092CFC205DD827BE5B59DB23E93A8A2A1F56858A448B2A2F0DE63E52CB8741".to_string(),
            rune_id: "1:18000".to_string(),
            is_issuer: true,
        };
        shared_local_repo
            .set_user_identifier_data(
                &user_identifier.user_uuid,
                &dkg_share_id,
                UserIdentifierData {
                    public_key: user_identifier.public_key.clone(),
                    rune_id: user_identifier.rune_id.clone(),
                    is_issuer: user_identifier.is_issuer,
                },
            )
            .await?;
        assert_eq!(
            Some(user_identifier.clone()),
            shared_local_repo
                .get_row_by_user_unique_id(&UserUniqueId {
                    uuid: user_identifier.user_uuid,
                    rune_id: user_identifier.rune_id.clone()
                })
                .await?
        );

        let deposit_addr_info = DepositAddrInfo {
            user_uuid: user_identifier.user_uuid,
            rune_id: user_identifier.rune_id,
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
            .set_deposit_addr_info(deposit_addr_info.clone())
            .await?;
        assert_eq!(
            Some(deposit_addr_info.clone()),
            shared_local_repo
                .get_deposit_addr_info(
                    &UserUniqueId {
                        uuid: deposit_addr_info.user_uuid,
                        rune_id: deposit_addr_info.rune_id.to_string()
                    },
                    deposit_addr_info.nonce.clone()
                )
                .await?
        );

        Ok(())
    }
}
