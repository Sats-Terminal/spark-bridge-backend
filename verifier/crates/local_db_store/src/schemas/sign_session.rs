use crate::storage::LocalDbStorage;
use async_trait::async_trait;
use frost::traits::SignerSignSessionStorage;
use frost::types::MusigId;
use frost::types::SignerSignData;
use frost::types::SignerSignState;
use frost::types::SigningMetadata;
use persistent_storage::error::DbError;
use sqlx::types::Json;
use uuid::Uuid;
use tracing::instrument;

#[async_trait]
impl SignerSignSessionStorage for LocalDbStorage {
    #[instrument(level = "trace", skip(self), ret)]
    async fn get_sign_data(&self, musig_id: &MusigId, session_id: Uuid) -> Result<Option<SignerSignData>, DbError> {
        let public_key = musig_id.get_public_key();
        let rune_id = musig_id.get_rune_id();

        let result: Option<(Json<SignerSignState>, Json<SigningMetadata>, Vec<u8>, Option<Vec<u8>>)> = sqlx::query_as(
            "SELECT sign_state, metadata, message_hash, tweak 
            FROM verifier.sign_session
            WHERE public_key = $1 AND rune_id = $2 AND session_id = $3",
        )
        .bind(public_key.to_string())
        .bind(rune_id)
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

    #[instrument(level = "trace", skip(self, sign_data), ret)]
    async fn set_sign_data(
        &self,
        musig_id: &MusigId,
        session_id: Uuid,
        sign_data: SignerSignData,
    ) -> Result<(), DbError> {
        let sign_state = Json(sign_data.sign_state);
        let public_key = musig_id.get_public_key();
        let rune_id = musig_id.get_rune_id();

        let _ = sqlx::query(
            "INSERT INTO verifier.sign_session (public_key, rune_id, session_id, sign_state, metadata, message_hash, tweak)
            VALUES ($1, $2, $3, $4, $5, $6, $7) ON CONFLICT (session_id) 
            DO UPDATE SET sign_state = $4",
        )
        .bind(public_key.to_string())
        .bind(rune_id)
        .bind(session_id.to_string())
        .bind(sign_state)
        .bind(Json(sign_data.metadata))
        .bind(sign_data.message_hash.clone())
        .bind(sign_data.tweak.clone())
        .execute(&self.get_conn().await?)
        .await
        .map_err(|e| DbError::BadRequest(e.to_string()))?;

        Ok(())
    }
}
