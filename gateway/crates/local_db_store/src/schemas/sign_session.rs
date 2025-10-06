use crate::storage::LocalDbStorage;
use async_trait::async_trait;
use frost::traits::AggregatorSignSessionStorage;
use frost::types::AggregatorSignState;
use frost::types::SigningMetadata;
use frost::types::AggregatorSignData;
use persistent_storage::error::DbError;
use persistent_storage::init::StorageHealthcheck;
use sqlx::types::Json;
use tracing::instrument;
use uuid::Uuid;

#[async_trait]
impl StorageHealthcheck for LocalDbStorage {
    async fn healthcheck(&self) -> Result<(), DbError> {
        self.postgres_repo.healthcheck().await
    }
}

#[async_trait]
impl AggregatorSignSessionStorage for LocalDbStorage {
    #[instrument(level = "trace", skip_all)]
    async fn get_sign_data(
        &self,
        dkg_share_id: &Uuid,
        session_id: Uuid,
    ) -> Result<Option<AggregatorSignData>, DbError> {
        let result: Option<(
            Json<AggregatorSignState>,
            Json<SigningMetadata>,
            Vec<u8>,
            Option<Vec<u8>>,
        )> = sqlx::query_as(
            "SELECT sign_state, aggregator_metadata, message_hash, tweak
            FROM gateway.sign_session
            WHERE dkg_share_id = $1 AND session_id = $2",
        )
        .bind(dkg_share_id)
        .bind(session_id.to_string())
        .fetch_optional(&self.get_conn().await?)
        .await
        .map_err(|e| DbError::BadRequest(e.to_string()))?;

        Ok(result.map(
            |(json_sign_state, json_metadata, message_hash, tweak)| AggregatorSignData {
                sign_state: json_sign_state.0,
                metadata: json_metadata.0,
                message_hash,
                tweak,
            },
        ))
    }

    #[instrument(level = "trace", skip_all)]
    async fn set_sign_data(
        &self,
        dkg_share_id: &Uuid,
        session_id: Uuid,
        sign_session_data: AggregatorSignData,
    ) -> Result<(), DbError> {
        let sign_state = Json(sign_session_data.sign_state);

        let _ = sqlx::query(
            "INSERT INTO gateway.sign_session (session_id, dkg_share_id, tweak, message_hash, aggregator_metadata, sign_state)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (session_id) DO UPDATE SET sign_state = $6",
        )
        .bind(session_id.to_string())
        .bind(dkg_share_id)
        .bind(sign_session_data.tweak.clone())
        .bind(sign_session_data.message_hash.clone())
        .bind(Json(sign_session_data.metadata))
        .bind(sign_state)
        .execute(&self.get_conn().await?)
        .await
        .map_err(|e| DbError::BadRequest(e.to_string()))?;

        Ok(())
    }
}
