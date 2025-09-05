use frost::traits::AggregatorUserSessionStorage;
use crate::storage::Storage;
use persistent_storage::error::DatabaseError;
use frost::types::AggregatorUserSessionInfo;
use bitcoin::secp256k1::PublicKey;
use async_trait::async_trait;
use sqlx::types::Json;
use uuid::Uuid;
use frost::types::SigningMetadata;
use frost::types::AggregatorUserSessionState;
use serde_json;

#[async_trait]
impl AggregatorUserSessionStorage for Storage {
    async fn get_session_info(&self, user_public_key: PublicKey, session_id: Uuid) -> Result<Option<AggregatorUserSessionInfo>, DatabaseError> {
        let result: Option<(Json<AggregatorUserSessionState>, Json<SigningMetadata>, Vec<u8>, Option<Vec<u8>>)> = sqlx::query_as("SELECT state_data, metadata, message_hash, tweak FROM user_session_info WHERE user_public_key = $1 AND session_id = $2")
            .bind(user_public_key.to_string())
            .bind(session_id.to_string())
            .fetch_optional(&self.get_conn().await?)
            .await
            .map_err(|e| DatabaseError::BadRequest(e.to_string()))?;

        Ok(result.map(|(state_data, metadata, message_hash, tweak)| AggregatorUserSessionInfo {
            state: state_data.0,
            metadata: metadata.0,
            message_hash,
            tweak,
        }))
    }

    async fn set_session_info(&self, user_public_key: PublicKey, session_id: Uuid, user_session_info: AggregatorUserSessionInfo) -> Result<(), DatabaseError> {
        let state_data = Json(user_session_info.state);

        let _ = sqlx::query("INSERT INTO user_session_info (user_public_key, session_id, state_data, metadata, message_hash, tweak) VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT (user_public_key, session_id) DO UPDATE SET state_data = $3, metadata = $4, message_hash = $5, tweak = $6")
            .bind(user_public_key.to_string())
            .bind(session_id.to_string())
            .bind(state_data)
            .bind(Json(user_session_info.metadata))
            .bind(user_session_info.message_hash.clone())
            .bind(user_session_info.tweak.clone())
            .execute(&self.get_conn().await?)
            .await
            .map_err(|e| DatabaseError::BadRequest(e.to_string()))?;

        Ok(())
    }
}
