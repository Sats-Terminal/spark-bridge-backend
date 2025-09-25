use crate::storage::LocalDbStorage;
use async_trait::async_trait;
use bitcoin::secp256k1::PublicKey;
use frost::traits::AggregatorDkgShareStorage;
use frost::types::AggregatorDkgShareData;
use frost::types::AggregatorDkgState;
use frost::types::MusigId;
use persistent_storage::error::DbError;
use sqlx::types::Json;
use std::str::FromStr;
use tracing::instrument;

#[async_trait]
impl AggregatorDkgShareStorage for LocalDbStorage {
    #[instrument(level = "trace", skip(self), ret)]
    async fn get_dkg_share_data(&self, musig_id: &MusigId) -> Result<Option<AggregatorDkgShareData>, DbError> {
        let public_key = musig_id.get_public_key();
        let rune_id = musig_id.get_rune_id();

        let result: Option<(Json<AggregatorDkgState>,)> = sqlx::query_as(
            "SELECT dkg_state 
            FROM gateway.musig_identifier 
            WHERE public_key = $1 AND rune_id = $2",
        )
        .bind(public_key.to_string())
        .bind(rune_id)
        .fetch_optional(&self.get_conn().await?)
        .await
        .map_err(|e| DbError::BadRequest(e.to_string()))?;

        Ok(result.map(|(json_dkg_state,)| AggregatorDkgShareData {
            dkg_state: json_dkg_state.0,
        }))
    }

    #[instrument(level = "trace", skip(self), ret)]
    async fn set_dkg_share_data(&self, musig_id: &MusigId, user_state: AggregatorDkgShareData) -> Result<(), DbError> {
        let dkg_state = Json(user_state.dkg_state);
        let public_key = musig_id.get_public_key();
        let rune_id = musig_id.get_rune_id();
        let is_issuer = matches!(musig_id, MusigId::Issuer { .. });

        let _ = sqlx::query(
            "INSERT INTO gateway.musig_identifier (public_key, rune_id, is_issuer, dkg_state) 
            VALUES ($1, $2, $3, $4) 
            ON CONFLICT (public_key, rune_id) DO UPDATE SET dkg_state = $4",
        )
        .bind(public_key.to_string())
        .bind(rune_id)
        .bind(is_issuer)
        .bind(dkg_state)
        .execute(&self.get_conn().await?)
        .await
        .map_err(|e| DbError::BadRequest(e.to_string()))?;

        Ok(())
    }

    async fn get_issuer_musig_id(&self, rune_id: String) -> Result<Option<MusigId>, DbError> {
        let result: Option<(String, String)> = sqlx::query_as(
            "SELECT public_key, rune_id 
            FROM gateway.musig_identifier 
            WHERE is_issuer = true AND rune_id = $1",
        )
        .bind(rune_id)
        .fetch_optional(&self.get_conn().await?)
        .await
        .map_err(|e| DbError::BadRequest(e.to_string()))?;

        match result {
            Some((public_key, rune_id)) => Ok(Some(MusigId::Issuer {
                issuer_public_key: PublicKey::from_str(&public_key).map_err(|e| DbError::BadRequest(e.to_string()))?,
                rune_id,
            })),
            None => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::make_repo_with_config;
    use bitcoin::secp256k1::{PublicKey, Secp256k1, SecretKey};
    use frost::aggregator::FrostAggregator;
    use frost::mocks::*;
    use frost::signer::FrostSigner;
    use frost::traits::SignerClient;
    use frost::traits::*;
    use frost::types::SigningMetadata;
    use frost::types::TokenTransactionMetadata;
    use frost_secp256k1_tr::Identifier;
    use frost_secp256k1_tr::keys::Tweak;
    use lrc20::token_transaction::{
        TokenTransaction, TokenTransactionCreateInput, TokenTransactionInput, TokenTransactionVersion,
    };
    use persistent_storage::init::{PostgresPool, PostgresRepo};
    use std::collections::BTreeMap;
    use std::sync::Arc;

    pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

    async fn create_signer(identifier: u16) -> FrostSigner {
        FrostSigner::new(
            identifier,
            Arc::new(MockSignerDkgShareIdStorage::new()),
            Arc::new(MockSignerSignSessionStorage::default()),
            3,
            2,
        )
    }

    async fn create_verifiers_map_easy() -> BTreeMap<Identifier, Arc<dyn SignerClient>> {
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

    fn create_signing_metadata() -> SigningMetadata {
        let token_transaction_metadata = TokenTransactionMetadata::PartialCreateToken {
            token_transaction: TokenTransaction {
                version: TokenTransactionVersion::V2,
                input: TokenTransactionInput::Create(TokenTransactionCreateInput {
                    issuer_public_key: PublicKey::from_secret_key(
                        &Secp256k1::new(),
                        &SecretKey::from_slice(&[1u8; 32]).unwrap(),
                    ),
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

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn test_aggregator_signer_integration(db: PostgresPool) -> anyhow::Result<()> {
        let storage = make_repo_with_config(db).await;

        let verifiers_map = create_verifiers_map_easy().await;
        let aggregator = FrostAggregator::new(verifiers_map, storage.clone(), storage);

        let secp = Secp256k1::new();
        let secret_key = SecretKey::from_slice(&[4u8; 32])?;
        let user_id = MusigId::User {
            user_public_key: PublicKey::from_secret_key(&secp, &secret_key),
            rune_id: "test_rune_id".to_string(),
        };

        //let user_id = "test_user";
        let message_hash = b"test_message";

        let public_key_package = aggregator.run_dkg_flow(&user_id).await?;

        let tweak = Some(b"test_tweak".as_slice());
        // let tweak = None;
        let metadata = create_signing_metadata();

        let signature = aggregator
            .run_signing_flow(user_id, message_hash, metadata, tweak)
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
