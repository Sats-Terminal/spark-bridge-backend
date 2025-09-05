use std::{collections::BTreeMap, sync::Arc};
use bitcoin::secp256k1::{Secp256k1, SecretKey, PublicKey};
use frost::{aggregator::FrostAggregator, mocks::*, signer::FrostSigner, traits::SignerClient};
use frost_secp256k1_tr::{keys::Tweak, Identifier};
use lrc20::token_transaction::{TokenTransaction, TokenTransactionVersion, TokenTransactionInput, TokenTransactionCreateInput};
use frost::types::{SigningMetadata, TokenTransactionMetadata};


fn create_signer(identifier: u16) -> FrostSigner {
    FrostSigner::new(
        identifier,
        Arc::new(MockSignerUserKeyStorage::new()),
        Arc::new(MockSignerSessionStorage::new()),
        3,
        2,
    )
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

fn create_signing_metadata() -> SigningMetadata {
    let token_transaction_metadata = TokenTransactionMetadata::PartialCreateToken {
        token_transaction: TokenTransaction {
            version: TokenTransactionVersion::V2,
            input: TokenTransactionInput::Create(TokenTransactionCreateInput {
                issuer_public_key: PublicKey::from_secret_key(&Secp256k1::new(), &SecretKey::from_slice(&[1u8; 32]).unwrap()),
                token_name: "test_token".to_string(),
                token_ticker: "TEST".to_string(),
                decimals: 8,
                max_supply: 1000000000000000000,
                is_freezable: false,
                creation_entity_public_key: None,
            }),
            leaves_to_create: vec![],
            spark_operator_identity_public_keys: vec![],
            expiry_time: 0,
            network: None,
            client_created_timestamp: 0,
        },
    };

    SigningMetadata {
        token_transaction_metadata,
    }
}

#[tokio::test]
async fn test_aggregator_signer_integration() {
    let verifiers_map = create_verifiers_map_easy();

    let aggregator = FrostAggregator::new(
        verifiers_map,
        Arc::new(MockAggregatorUserKeyStorage::new()),
        Arc::new(MockAggregatorUserSessionStorage::new()),
    );

    let secp = Secp256k1::new();
    let secret_key = SecretKey::from_slice(&[1u8; 32]).unwrap();
    let user_id = PublicKey::from_secret_key(&secp, &secret_key);

    //let user_id = "test_user";
    let message_hash = b"test_message";
    // let tweak = Some(b"test_tweak".as_slice());
    let tweak = None;

    let public_key_package = aggregator.run_dkg_flow(user_id).await.unwrap();
    let metadata = create_signing_metadata();

    let signature = aggregator
        .run_signing_flow(user_id, message_hash, metadata, tweak)
        .await
        .unwrap();

    let tweaked_public_key_package = match tweak.clone() {
        Some(tweak) => public_key_package.clone().tweak(Some(tweak.to_vec())),
        None => public_key_package.clone(),
    };
    tweaked_public_key_package
        .verifying_key()
        .verify(message_hash, &signature)
        .unwrap();
}

#[tokio::test]
async fn test_parallel_signing_sessions_via_aggregator() {
    let verifiers_map = create_verifiers_map_easy();

    let aggregator = FrostAggregator::new(
        verifiers_map,
        Arc::new(MockAggregatorUserKeyStorage::new()),
        Arc::new(MockAggregatorUserSessionStorage::new()),
    );

    let secp = Secp256k1::new();
    let secret_key = SecretKey::from_slice(&[1u8; 32]).unwrap();
    let user_id = PublicKey::from_secret_key(&secp, &secret_key);
    //let user_id = "test_user".to_string();
    let msg_a = b"parallel message A".to_vec();
    let msg_b = b"parallel message B".to_vec();
    let tweak = None::<&[u8]>;

    let public_key_package = aggregator.run_dkg_flow(user_id.clone()).await.unwrap();
    let metadata = create_signing_metadata();

    let (sig_res_a, sig_res_b) = tokio::join!(
        aggregator.run_signing_flow(user_id.clone(), msg_a.as_slice(), metadata.clone(), tweak),
        aggregator.run_signing_flow(user_id.clone(), msg_b.as_slice(), metadata, tweak),
    );

    let signature_a = sig_res_a.unwrap();
    let signature_b = sig_res_b.unwrap();

    let pk = public_key_package.clone();
    pk.verifying_key()
        .verify(msg_a.as_slice(), &signature_a)
        .expect("signature A must be valid");

    pk.verifying_key()
        .verify(msg_b.as_slice(), &signature_b)
        .expect("signature B must be valid");

    assert_ne!(signature_a, signature_b, "signatures for different messages should differ");
}
