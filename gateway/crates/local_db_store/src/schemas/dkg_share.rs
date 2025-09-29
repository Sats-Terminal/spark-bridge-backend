use crate::storage::LocalDbStorage;
use async_trait::async_trait;
use frost::types::{AggregatorDkgState, DkgShareId};
use persistent_storage::error::DbError;
use sqlx::types::Json;
use uuid::Uuid;

use frost::traits::AggregatorDkgShareStorage;
use frost::types::AggregatorDkgShareData;
use std::str::FromStr;
use tracing::instrument;

#[async_trait]
pub trait DkgShareGenerate {
    async fn generate_dkg_share_entity(&self) -> Result<DkgShareId, DbError>;
    async fn get_random_unused_dkg_share(&self) -> Result<DkgShareId, DbError>;
    async fn count_unused_dkg_shares(&self) -> Result<u64, DbError>;
}

#[async_trait]
impl DkgShareGenerate for LocalDbStorage {
    async fn generate_dkg_share_entity(&self) -> Result<DkgShareId, DbError> {
        let result: (Uuid,) =
            sqlx::query_as("INSERT INTO gateway.dkg_share (dkg_state) VALUES ($1) RETURNING dkg_share_id;")
                .bind(Json(AggregatorDkgState::Initialized))
                .fetch_one(&self.get_conn().await?)
                .await
                .map_err(|e| DbError::BadRequest(e.to_string()))?;
        Ok(result.0)
    }

    async fn get_random_unused_dkg_share(&self) -> Result<DkgShareId, DbError> {
        todo!()
    }

    async fn count_unused_dkg_shares(&self) -> Result<u64, DbError> {
        todo!()
    }
}

#[async_trait]
impl AggregatorDkgShareStorage for LocalDbStorage {
    #[instrument(level = "trace", skip(self), ret)]
    async fn get_dkg_share_data(&self, dkg_share_id: &DkgShareId) -> Result<Option<AggregatorDkgShareData>, DbError> {
        let result: Option<(Json<AggregatorDkgState>,)> = sqlx::query_as(
            "SELECT dkg_state 
            FROM gateway.dkg_share
            WHERE dkg_share_id = $1",
        )
        .bind(dkg_share_id)
        .fetch_optional(&self.get_conn().await?)
        .await
        .map_err(|e| DbError::BadRequest(e.to_string()))?;

        Ok(result.map(|(json_dkg_state,)| AggregatorDkgShareData {
            dkg_state: json_dkg_state.0,
        }))
    }

    #[instrument(level = "trace", skip(self), ret)]
    async fn set_dkg_share_data(
        &self,
        dkg_share_id: &DkgShareId,
        dkg_share_data: AggregatorDkgShareData,
    ) -> Result<(), DbError> {
        let _ = sqlx::query(
            "INSERT INTO gateway.dkg_share (dkg_share_id, dkg_state)
            VALUES ($1, $2)
            ON CONFLICT (dkg_share_id) DO UPDATE SET dkg_state = $2",
        )
        .bind(dkg_share_id)
        .bind(Json(dkg_share_data.dkg_state))
        .execute(&self.get_conn().await?)
        .await
        .map_err(|e| DbError::BadRequest(e.to_string()))?;

        Ok(())
    }
}
