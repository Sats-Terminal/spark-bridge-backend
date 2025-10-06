use crate::storage::LocalDbStorage;
use async_trait::async_trait;
use frost::traits::SignerSignSessionStorage;
use frost::types::SignerSignData;
use frost::types::SignerSignState;
use frost::types::SigningMetadata;
use persistent_storage::error::DbError;
use sqlx::types::Json;
use tracing::instrument;
use uuid::Uuid;

#[async_trait]
impl SignerSignSessionStorage for LocalDbStorage {
    #[instrument(level = "trace", skip(self), ret)]
    async fn get_sign_data(
        &self,
        dkg_share_id: &Uuid,
        session_id: Uuid,
    ) -> Result<Option<SignerSignData>, DbError> {
        let result: Option<(Json<SignerSignState>, Json<SigningMetadata>, Vec<u8>, Option<Vec<u8>>)> = sqlx::query_as(
            "SELECT sign_state, metadata, message_hash, tweak
            FROM verifier.sign_session
            WHERE dkg_share_id = $1 AND session_id = $2",
        )
        .bind(dkg_share_id)
        .bind(session_id.to_string())
        .fetch_optional(&self.get_conn().await?)
        .await
        .map_err(|e| DbError::BadRequest(e.to_string()))?;

        Ok(
            result.map(|(json_sign_state, json_metadata, message_hash, tweak)| SignerSignData {
                sign_state: json_sign_state.0,
                metadata: json_metadata.0,
                message_hash,
                tweak,
            }),
        )
    }

    #[instrument(level = "trace", skip(self, sign_session_data), ret)]
    async fn set_sign_data(
        &self,
        dkg_share_id: &Uuid,
        session_id: Uuid,
        sign_session_data: SignerSignData,
    ) -> Result<(), DbError> {
        let _ = sqlx::query(
            "INSERT INTO verifier.sign_session (session_id, dkg_share_id, tweak, message_hash, metadata, sign_state)
            VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT (session_id)
            DO UPDATE SET sign_state = $6",
        )
        .bind(session_id)
        .bind(*dkg_share_id)
        .bind(sign_session_data.tweak)
        .bind(sign_session_data.message_hash)
        .bind(Json(sign_session_data.metadata))
        .bind(Json(sign_session_data.sign_state))
        .execute(&self.get_conn().await?)
        .await
        .map_err(|e| DbError::BadRequest(e.to_string()))?;
        Ok(())
    }
}
