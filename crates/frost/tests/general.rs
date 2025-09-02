use std::{collections::BTreeMap, sync::Arc};

use frost::{aggregator::FrostAggregator, config::*, mocks::*, signer::FrostSigner, traits::SignerClient};
use frost_secp256k1_tr::{Identifier, keys::Tweak};

fn create_signer(identifier: u16) -> FrostSigner {
    FrostSigner::new(
        SignerConfig {
            identifier,
            threshold: 2,
            total_participants: 3,
        },
        Arc::new(MockSignerUserStorage::new()),
    )
}

#[tokio::test]
async fn test_aggregator_signer_integration() {
    let signer1 = create_signer(1);
    let signer2 = create_signer(2);
    let signer3 = create_signer(3);

    let mock_signer_client1 = MockSignerClient::new(signer1);
    let mock_signer_client2 = MockSignerClient::new(signer2);
    let mock_signer_client3 = MockSignerClient::new(signer3);

    let identifier_1: Identifier = 1.try_into().unwrap();
    let identifier_2: Identifier = 2.try_into().unwrap();
    let identifier_3: Identifier = 3.try_into().unwrap();
    let verifiers_map = BTreeMap::from([
        (identifier_1, Arc::new(mock_signer_client1) as Arc<dyn SignerClient>),
        (identifier_2, Arc::new(mock_signer_client2) as Arc<dyn SignerClient>),
        (identifier_3, Arc::new(mock_signer_client3) as Arc<dyn SignerClient>),
    ]);

    let aggregator = FrostAggregator::new(
        AggregatorConfig {
            threshold: 2,
            total_participants: 3,
            verifier_identifiers: vec![1, 2, 3],
        },
        verifiers_map,
        Arc::new(MockAggregatorUserStorage::new()),
    );

    let user_id = "test_user";
    let message = b"test_message";
    let tweak = b"test_tweak";

    let public_key_package = aggregator.run_dkg_flow(user_id.to_string()).await.unwrap();
    let signature = aggregator
        .run_signing_flow(user_id.to_string(), message, tweak)
        .await
        .unwrap();

    let tweaked_public_key_package = public_key_package.clone().tweak(Some(tweak.to_vec()));
    tweaked_public_key_package
        .verifying_key()
        .verify(message, &signature)
        .unwrap();
}
