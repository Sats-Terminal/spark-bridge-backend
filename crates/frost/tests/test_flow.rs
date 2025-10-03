mod tests {
    use bitcoin::key::TapTweak;
    use bitcoin::key::UntweakedPublicKey;
    use bitcoin::secp256k1::{PublicKey, Secp256k1};
    use frost::traits::AggregatorDkgShareStorage;
    use frost::types::{AggregatorDkgShareData, AggregatorDkgState, DkgShareId, SigningMetadata, TweakBytes};
    use frost::utils::generate_tweak_bytes;
    use frost::{aggregator::FrostAggregator, mocks::*, signer::FrostSigner, traits::SignerClient};
    use frost_secp256k1_tr::{Identifier, keys::Tweak};
    use global_utils::common_types::get_uuid;
    use std::str::FromStr;
    use std::{collections::BTreeMap, sync::Arc};

    #[tokio::test]
    async fn test_aggregator_signer_integration() -> anyhow::Result<()> {
        let msg_hash = b"test_message";
        _test_aggregator_signer_integration(msg_hash, None).await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_aggregator_signer_integration_tweaked() -> anyhow::Result<()> {
        let msg_hash = b"test_message";
        let tweak = generate_tweak_bytes();
        _test_aggregator_signer_integration(msg_hash, Some(tweak)).await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_parallel_signing_sessions_via_aggregator() -> anyhow::Result<()> {
        let msg_a = b"parallel message A".to_vec();
        let msg_b = b"parallel message B".to_vec();
        _test_parallel_signing_sessions_via_aggregator(&msg_a, &msg_b, None).await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_parallel_signing_sessions_via_aggregator_tweaked() -> anyhow::Result<()> {
        let msg_a = b"parallel message A".to_vec();
        let msg_b = b"parallel message B".to_vec();
        let tweak = generate_tweak_bytes();
        _test_parallel_signing_sessions_via_aggregator(&msg_a, &msg_b, Some(tweak)).await?;
        Ok(())
    }

    #[test]
    fn test_get_tweaked_public_key() {
        let ctx = Secp256k1::new();

        let our_public_key =
            PublicKey::from_str("038144ac71b61ab0e0a56967696a4f31a0cdd492cd3753d59aa978e0c8eaa5a60e").unwrap();

        let untweaked_public_key: UntweakedPublicKey = our_public_key.into();
        let (_tweaked_public_key, _) = untweaked_public_key.tap_tweak(&ctx, None);
    }

    async fn _test_parallel_signing_sessions_via_aggregator(
        msg_hash_a: &[u8],
        msg_hash_b: &[u8],
        tweak: Option<TweakBytes>,
    ) -> anyhow::Result<()> {
        let verifiers_map = create_verifiers_map_easy();

        let dkg_share_id: DkgShareId = get_uuid();
        let agg_storage = MockAggregatorDkgShareIdStorage::default();
        agg_storage
            .set_dkg_share_agg_data(
                &dkg_share_id,
                AggregatorDkgShareData {
                    dkg_state: AggregatorDkgState::Initialized,
                },
            )
            .await?;

        let aggregator = FrostAggregator::new(
            verifiers_map,
            Arc::new(agg_storage),
            Arc::new(MockAggregatorSignSessionStorage::default()),
        );

        let public_key_package = aggregator.run_dkg_flow(&dkg_share_id).await?;
        let metadata = SigningMetadata::Authorization;

        let (sig_res_a, sig_res_b) = tokio::join!(
            aggregator.run_signing_flow(dkg_share_id.clone(), msg_hash_a, metadata.clone(), tweak),
            aggregator.run_signing_flow(dkg_share_id.clone(), msg_hash_b, metadata, tweak),
        );

        let signature_a = sig_res_a?;
        let signature_b = sig_res_b?;

        let tweaked_public_key_package = match tweak.clone() {
            Some(tweak) => public_key_package.clone().tweak(Some(tweak.to_vec())),
            None => public_key_package.clone(),
        };
        tweaked_public_key_package
            .verifying_key()
            .verify(msg_hash_a, &signature_a)
            .expect("signature A must be valid");
        tweaked_public_key_package
            .verifying_key()
            .verify(msg_hash_b, &signature_b)
            .expect("signature B must be valid");

        assert_ne!(
            signature_a, signature_b,
            "signatures for different messages should differ"
        );
        Ok(())
    }

    async fn _test_aggregator_signer_integration(msg_hash: &[u8], tweak: Option<TweakBytes>) -> anyhow::Result<()> {
        let verifiers_map = create_verifiers_map_easy();

        let dkg_share_id: DkgShareId = get_uuid();
        let agg_storage = MockAggregatorDkgShareIdStorage::default();
        agg_storage
            .set_dkg_share_agg_data(
                &dkg_share_id,
                AggregatorDkgShareData {
                    dkg_state: AggregatorDkgState::Initialized,
                },
            )
            .await?;

        let aggregator = FrostAggregator::new(
            verifiers_map,
            Arc::new(agg_storage),
            Arc::new(MockAggregatorSignSessionStorage::default()),
        );

        let public_key_package = aggregator.run_dkg_flow(&dkg_share_id).await?;
        let metadata = SigningMetadata::Authorization;

        let signature = aggregator
            .run_signing_flow(dkg_share_id.clone(), msg_hash, metadata, tweak)
            .await?;

        let tweaked_public_key_package = match tweak.clone() {
            Some(tweak) => public_key_package.clone().tweak(Some(tweak.to_vec())),
            None => public_key_package.clone(),
        };
        tweaked_public_key_package
            .verifying_key()
            .verify(msg_hash, &signature)?;
        Ok(())
    }

    fn create_signer(identifier: u16) -> FrostSigner {
        FrostSigner::new(
            identifier,
            Arc::new(MockSignerDkgShareIdStorage::default()),
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
}
