use frost::traits::SignerUserKeyStorage;
use crate::storage::Storage;
use persistent_storage::error::DatabaseError;
use frost::types::SignerUserKeyInfo;
use frost::types::SignerUserKeyState;
use bitcoin::secp256k1::PublicKey;
use async_trait::async_trait;
use sqlx::types::Json;


#[async_trait]
impl SignerUserKeyStorage for Storage {
    async fn get_key_info(&self, user_public_key: PublicKey) -> Result<Option<SignerUserKeyInfo>, DatabaseError> {
        let result: Option<(Json<SignerUserKeyState>,)> = sqlx::query_as("SELECT state_data FROM user_key_info WHERE user_public_key = $1")
            .bind(user_public_key.to_string())
            .fetch_optional(&self.get_conn().await?)
            .await
            .map_err(|e| DatabaseError::BadRequest(e.to_string()))?;
        
        Ok(result.map(|(state_data,)| SignerUserKeyInfo {
            state: state_data.0,
        }))
    }

    async fn set_key_info(&self, user_public_key: PublicKey, user_state: SignerUserKeyInfo) -> Result<(), DatabaseError> {
        let state_data = Json(user_state.state);

        let _ = sqlx::query("INSERT INTO user_key_info (user_public_key, state_data) VALUES ($1, $2) ON CONFLICT (user_public_key) DO UPDATE SET state_data = $2")
            .bind(user_public_key.to_string())
            .bind(state_data)
            .execute(&self.get_conn().await?)
            .await
            .map_err(|e| DatabaseError::BadRequest(e.to_string()))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use frost::aggregator::FrostAggregator;
    use frost::mocks::*;
    use frost::traits::SignerClient;
    use frost::signer::FrostSigner;
    use frost::types::SigningMetadata;
    use frost::types::TokenTransactionMetadata;
    use std::sync::Arc;
    use frost_secp256k1_tr::Identifier;
    use std::collections::BTreeMap;
    use lrc20::token_transaction::{TokenTransaction, TokenTransactionVersion, TokenTransactionInput, TokenTransactionCreateInput};
    use bitcoin::secp256k1::{Secp256k1, SecretKey, PublicKey};
    use frost_secp256k1_tr::keys::Tweak;
    use frost::traits::SignerUserSessionStorage;

    async fn create_signer(identifier: u16, is_mock_key_storage: bool, is_mock_session_storage: bool) -> FrostSigner {
        let storage = Storage::new("postgres://admin_manager:password@localhost:5471/production_db_name".to_string()).await.unwrap();
        let arc_storage = Arc::new(storage);

        let user_key_storage: Arc<dyn SignerUserKeyStorage> = if is_mock_key_storage {
            Arc::new(MockSignerUserKeyStorage::new())
        } else {
            arc_storage.clone()
        };

        let user_session_storage: Arc<dyn SignerUserSessionStorage> = if is_mock_session_storage {
            Arc::new(MockSignerSessionStorage::new())
        } else {
            arc_storage
        };

        FrostSigner::new(
            identifier,
            user_key_storage,
            user_session_storage,
            3,
            2,
        )
    }
    
    async fn create_verifiers_map_easy() -> BTreeMap<Identifier, Arc<dyn SignerClient>> {
        let signer1 = create_signer(1, true, true).await;
        let signer2 = create_signer(2, true, true).await;
        let signer3 = create_signer(3, false, false).await;
    
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
        let verifiers_map = create_verifiers_map_easy().await;
    
        let aggregator = FrostAggregator::new(
            verifiers_map,
            Arc::new(MockAggregatorUserKeyStorage::new()),
            Arc::new(MockAggregatorUserSessionStorage::new()),
        );
    
        let secp = Secp256k1::new();
        let secret_key = SecretKey::from_slice(&[4u8; 32]).unwrap();
        let user_id = PublicKey::from_secret_key(&secp, &secret_key);
    
        //let user_id = "test_user";
        let message_hash = b"test_message";
    
        let public_key_package = aggregator.run_dkg_flow(user_id).await.unwrap();

        let tweak = Some(b"test_tweak".as_slice());
        // let tweak = None;
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
    
}