mod utils;

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, sync::Arc};

    use bitcoin::secp256k1::{PublicKey, Secp256k1, SecretKey};
    use frost::{
        aggregator::FrostAggregator,
        mocks::*,
        signer::FrostSigner,
        traits::{AggregatorDkgShareStorage, SignerClient, SignerDkgShareStorage, SignerSignSessionStorage},
        types::{AggregatorDkgShareData, AggregatorDkgState, SigningMetadata, TweakBytes},
        utils::generate_tweak_bytes,
    };
    use frost_secp256k1_tr::{Identifier, keys::Tweak};
    use global_utils::common_types::get_uuid;
    use persistent_storage::init::{PostgresPool, PostgresRepo};
    use tracing::{debug, instrument};
    use verifier_local_db_store::storage::LocalDbStorage;

    use super::*;
    use crate::utils::common::{MIGRATOR, TEST_LOGGER};

    #[instrument]
    #[sqlx::test(migrator = "MIGRATOR")]
    async fn test_signer_integration(db: PostgresPool) -> eyre::Result<()> {
        let _logger_guard = &*TEST_LOGGER;

        let local_repo = LocalDbStorage {
            postgres_repo: PostgresRepo { pool: db },
        };
        let shared_local_repo = Arc::new(local_repo);
        let msg_hash = b"hello_world!";
        let tweak = None;
        _test_aggregator_signer_integration(msg_hash, tweak, shared_local_repo).await?;
        Ok(())
    }

    #[instrument]
    #[sqlx::test(migrator = "MIGRATOR")]
    async fn test_signer_integration_tweaked(db: PostgresPool) -> eyre::Result<()> {
        let _logger_guard = &*TEST_LOGGER;

        let local_repo = LocalDbStorage {
            postgres_repo: PostgresRepo { pool: db },
            // postgres_repo: PostgresRepo::from_config(PostgresDbCredentials::from_db_url()?).await?,
        };
        let shared_local_repo = Arc::new(local_repo);
        let msg_hash = b"hello_world!";
        let tweak = Some(generate_tweak_bytes());
        _test_aggregator_signer_integration(msg_hash, tweak, shared_local_repo).await?;
        Ok(())
    }

    #[inline]
    async fn create_mocked_signer(
        identifier: u16,
        is_mock_key_storage: bool,
        is_mock_session_storage: bool,
        total_participants: u16,
        threshold: u16,
        shared_local_storage: Arc<LocalDbStorage>,
    ) -> FrostSigner {
        let user_key_storage: Arc<dyn SignerDkgShareStorage> = if is_mock_key_storage {
            Arc::new(MockSignerDkgShareIdStorage::default())
        } else {
            shared_local_storage.clone()
        };

        let user_session_storage: Arc<dyn SignerSignSessionStorage> = if is_mock_session_storage {
            Arc::new(MockSignerSignSessionStorage::default())
        } else {
            shared_local_storage
        };
        FrostSigner::new(identifier, user_key_storage, user_session_storage, 3, 2).unwrap()
    }

    #[inline]
    async fn create_real_signer(
        identifier: u16,
        total_participants: u16,
        threshold: u16,
        shared_local_storage: Arc<LocalDbStorage>,
    ) -> FrostSigner {
        FrostSigner::new(
            identifier,
            shared_local_storage.clone(),
            shared_local_storage.clone(),
            total_participants,
            threshold,
        )
        .unwrap()
    }

    async fn create_verifiers_map_easy(
        local_db_storage: Arc<LocalDbStorage>,
    ) -> BTreeMap<Identifier, Arc<dyn SignerClient>> {
        let signer1 = create_mocked_signer(1, true, true, 3, 2, local_db_storage.clone()).await;
        let signer2 = create_mocked_signer(2, true, true, 3, 2, local_db_storage.clone()).await;
        let signer3 = create_real_signer(3, 3, 2, local_db_storage).await;

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

    #[instrument]
    async fn _test_aggregator_signer_integration(
        msg_hash: &[u8],
        tweak: Option<TweakBytes>,
        local_db_storage: Arc<LocalDbStorage>,
    ) -> eyre::Result<()> {
        let dkg_share_id = get_uuid();
        // let dkg_share_id = local_db_storage.generate_dkg_share_entity().await?;

        debug!("Verifiers ...");
        let verifiers_map = create_verifiers_map_easy(local_db_storage.clone()).await;
        let mock_dkg_share_storage = MockAggregatorDkgShareIdStorage::default();
        mock_dkg_share_storage
            .set_dkg_share_agg_data(
                &dkg_share_id,
                AggregatorDkgShareData {
                    dkg_state: AggregatorDkgState::Initialized,
                },
            )
            .await?;

        debug!("aggregator ...");
        let aggregator = FrostAggregator::new(
            verifiers_map,
            Arc::new(mock_dkg_share_storage),
            Arc::new(MockAggregatorSignSessionStorage::default()),
        );

        let secp = Secp256k1::new();
        let secret_key = SecretKey::from_slice(&[4u8; 32])?;

        debug!("pubkeypacage ...");

        let public_key_package = aggregator.run_dkg_flow(&dkg_share_id).await?;

        let metadata = SigningMetadata::Authorization;

        debug!("signing flow ...");

        let signature = aggregator
            .run_signing_flow(dkg_share_id, msg_hash, metadata, tweak, false)
            .await?;

        debug!("tweak package ...");

        let tweaked_public_key_package = match tweak.clone() {
            Some(tweak) => public_key_package.clone().tweak(Some(tweak.to_vec())),
            None => public_key_package.clone(),
        };
        tweaked_public_key_package
            .verifying_key()
            .verify(msg_hash, &signature)?;
        Ok(())
    }
}
