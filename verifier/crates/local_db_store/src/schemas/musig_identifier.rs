use crate::storage::LocalDbStorage;
use async_trait::async_trait;
use frost::traits::SignerMusigIdStorage;
use frost::types::MusigId;
use frost::types::SignerDkgState;
use frost::types::SignerMusigIdData;
use persistent_storage::error::DbError;
use sqlx::types::Json;

#[async_trait]
impl SignerMusigIdStorage for LocalDbStorage {
    async fn get_musig_id_data(&self, musig_id: &MusigId) -> Result<Option<SignerMusigIdData>, DbError> {
        let public_key = musig_id.get_public_key();
        let rune_id = musig_id.get_rune_id();

        let result: Option<(Json<SignerDkgState>,)> = sqlx::query_as(
            "SELECT dkg_state 
            FROM verifier.musig_identifier 
            WHERE public_key = $1 AND rune_id = $2",
        )
        .bind(public_key.to_string())
        .bind(rune_id)
        .fetch_optional(&self.get_conn().await?)
        .await
        .map_err(|e| DbError::BadRequest(e.to_string()))?;

        Ok(result.map(|(json_dkg_state,)| SignerMusigIdData {
            dkg_state: json_dkg_state.0,
        }))
    }

    async fn set_musig_id_data(&self, musig_id: &MusigId, musig_id_data: SignerMusigIdData) -> Result<(), DbError> {
        let dkg_state = Json(musig_id_data.dkg_state);
        let public_key = musig_id.get_public_key();
        let rune_id = musig_id.get_rune_id();
        let is_issuer = matches!(musig_id, MusigId::Issuer { .. });

        let _ = sqlx::query(
            "INSERT INTO verifier.musig_identifier (public_key, rune_id, is_issuer, dkg_state) 
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::secp256k1::{PublicKey, Secp256k1, SecretKey};
    use frost::aggregator::FrostAggregator;
    use frost::mocks::*;
    use frost::signer::FrostSigner;
    use frost::traits::SignerClient;
    use frost::traits::SignerMusigIdStorage;
    use frost::traits::SignerSignSessionStorage;
    use frost::types::SigningMetadata;
    use frost::utils::generate_tweak_bytes;
    use frost_secp256k1_tr::Identifier;
    use frost_secp256k1_tr::keys::Tweak;
    use lrc20::token_transaction::{
        TokenTransaction, TokenTransactionCreateInput, TokenTransactionInput, TokenTransactionVersion,
    };
    use std::collections::BTreeMap;
    use std::sync::Arc;

    async fn create_signer(identifier: u16, is_mock_key_storage: bool, is_mock_session_storage: bool) -> FrostSigner {
        let storage =
            LocalDbStorage::new("postgres://admin_manager:password@localhost:5471/production_db_name".to_string())
                .await
                .unwrap();
        let arc_storage = Arc::new(storage);

        let user_key_storage: Arc<dyn SignerMusigIdStorage> = if is_mock_key_storage {
            Arc::new(MockSignerMusigIdStorage::default())
        } else {
            arc_storage.clone()
        };

        let user_session_storage: Arc<dyn SignerSignSessionStorage> = if is_mock_session_storage {
            Arc::new(MockSignerSignSessionStorage::default())
        } else {
            arc_storage
        };

        FrostSigner::new(identifier, user_key_storage, user_session_storage, 3, 2)
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

    #[tokio::test]
    async fn test_aggregator_signer_integration() {
        let verifiers_map = create_verifiers_map_easy().await;

        let aggregator = FrostAggregator::new(
            verifiers_map,
            Arc::new(MockAggregatorMusigIdStorage::default()),
            Arc::new(MockAggregatorSignSessionStorage::default()),
        );

        let secp = Secp256k1::new();
        let secret_key = SecretKey::from_slice(&[4u8; 32]).unwrap();
        let user_id = MusigId::User {
            user_public_key: PublicKey::from_secret_key(&secp, &secret_key),
            rune_id: "test_rune_id".to_string(),
        };

        //let user_id = "test_user";
        let message_hash = b"test_message";

        let public_key_package = aggregator.run_dkg_flow(&user_id).await.unwrap();

        let tweak = Some(generate_tweak_bytes());
        // let tweak = None;
        let metadata = SigningMetadata::Authorization;

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
