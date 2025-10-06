use crate::storage::LocalDbStorage;
use async_trait::async_trait;
use frost::types::{AggregatorDkgState, DkgShareId};
use persistent_storage::error::DbError;
use sqlx::types::Json;
use uuid::Uuid;

use crate::schemas::user_identifier::{UserIdentifierData, UserIds};
use frost::traits::AggregatorDkgShareStorage;
use frost::types::AggregatorDkgShareData;
use global_utils::common_types::get_uuid;
use persistent_storage::init::PersistentRepoTrait;
use sqlx::Acquire;
use thiserror::Error;
use tracing::instrument;
// TODO: think about, maybe add another field that would be as flag, whether the entry were used or not

#[derive(Debug, Error)]
pub enum DkgShareGenerateError {
    #[error(transparent)]
    DbError(#[from] DbError),
    #[error("No available spare finalized dkgs, waiting verification from verifiers")]
    DkgPregenFailed,
}

#[async_trait]
pub trait DkgShareGenerate {
    /// Generated dkg share entity in Aggregator side with state `AggregatorDkgState::Initialized`
    async fn generate_dkg_share_entity(&self) -> Result<DkgShareId, DbError>;
    /// Returns unused dkg share uuid to user and assigns at the same time user identifier to this user
    async fn get_random_unused_dkg_share(&self, data: UserIdentifierData) -> Result<UserIds, DkgShareGenerateError>;
    async fn count_unused_dkg_shares(&self) -> Result<u64, DbError>;
    async fn count_unused_finalized_dkg_shares(&self) -> Result<u64, DbError>;
}

#[async_trait]
impl DkgShareGenerate for LocalDbStorage {
    #[instrument(level = "trace", skip_all, ret)]
    async fn generate_dkg_share_entity(&self) -> Result<DkgShareId, DbError> {
        let result: (Uuid,) =
            sqlx::query_as("INSERT INTO gateway.dkg_share (dkg_aggregator_state) VALUES ($1) RETURNING dkg_share_id;")
                .bind(Json(AggregatorDkgState::Initialized))
                .fetch_one(&self.get_conn().await?)
                .await
                .map_err(|e| DbError::BadRequest(e.to_string()))?;
        Ok(result.0)
    }

    #[instrument(level = "debug", skip_all, ret)]
    async fn get_random_unused_dkg_share(&self, data: UserIdentifierData) -> Result<UserIds, DkgShareGenerateError> {
        let mut conn = self.postgres_repo.get_conn().await?;
        let mut transaction = conn.begin().await.map_err(|e| DbError::from(e))?;

        let dkg_share_id: Option<(DkgShareId,)> = sqlx::query_as(
            "SELECT ds.dkg_share_id
                FROM gateway.dkg_share ds
                LEFT JOIN gateway.user_identifier ui ON ds.dkg_share_id = ui.dkg_share_id
                WHERE ui.dkg_share_id IS NULL
                  AND ds.dkg_aggregator_state::text ILIKE '%DkgFinalized%'
                ORDER BY ds.dkg_share_id DESC
                LIMIT 1;",
        )
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|e| DbError::BadRequest(e.to_string()))?;

        if dkg_share_id.is_none() {
            return Err(DkgShareGenerateError::DkgPregenFailed);
        }

        let dkg_share_id = dkg_share_id.unwrap().0;
        let user_uuid = get_uuid();

        let _ = sqlx::query(
            "INSERT INTO gateway.user_identifier (user_uuid, dkg_share_id, public_key, rune_id, is_issuer)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (user_uuid, rune_id) DO NOTHING",
        )
        .bind(user_uuid)
        .bind(dkg_share_id)
        .bind(data.public_key)
        .bind(data.rune_id.clone())
        .bind(data.is_issuer)
        .execute(&mut *transaction)
        .await
        .map_err(|e| DbError::BadRequest(e.to_string()))?;

        transaction.commit().await.map_err(|e| DbError::from(e))?;
        Ok(UserIds {
            user_uuid,
            dkg_share_id,
            rune_id: data.rune_id,
        })
    }

    #[instrument(level = "trace", skip_all, ret)]
    async fn count_unused_dkg_shares(&self) -> Result<u64, DbError> {
        let result: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) AS unused_dkg_share_count
                                FROM gateway.dkg_share ds
                                LEFT JOIN gateway.user_identifier ui ON ds.dkg_share_id = ui.dkg_share_id
                                WHERE ui.dkg_share_id IS NULL;",
        )
        .bind(Json(AggregatorDkgState::Initialized))
        .fetch_one(&self.get_conn().await?)
        .await
        .map_err(|e| DbError::BadRequest(e.to_string()))?;
        Ok(result.0 as u64)
    }

    #[instrument(level = "trace", skip_all, ret)]
    async fn count_unused_finalized_dkg_shares(&self) -> Result<u64, DbError> {
        let result: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) AS unused_dkg_share_count
                                FROM gateway.dkg_share ds
                                LEFT JOIN gateway.user_identifier ui ON ds.dkg_share_id = ui.dkg_share_id
                                    WHERE ds.dkg_aggregator_state::text ILIKE '%DkgFinalized%'
                                AND ui.dkg_share_id IS NULL;",
        )
        .bind(Json(AggregatorDkgState::Initialized))
        .fetch_one(&self.get_conn().await?)
        .await
        .map_err(|e| DbError::BadRequest(e.to_string()))?;
        Ok(result.0 as u64)
    }
}

#[async_trait]
impl AggregatorDkgShareStorage for LocalDbStorage {
    #[instrument(level = "trace", skip(self), ret)]
    async fn get_dkg_share_agg_data(
        &self,
        dkg_share_id: &DkgShareId,
    ) -> Result<Option<AggregatorDkgShareData>, DbError> {
        let result: Option<(Json<AggregatorDkgState>,)> = sqlx::query_as(
            "SELECT dkg_aggregator_state
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
    async fn set_dkg_share_agg_data(
        &self,
        dkg_share_id: &DkgShareId,
        dkg_share_data: AggregatorDkgShareData,
    ) -> Result<(), DbError> {
        let _ = sqlx::query(
            "INSERT INTO gateway.dkg_share (dkg_share_id, dkg_aggregator_state)
            VALUES ($1, $2)
            ON CONFLICT (dkg_share_id) DO UPDATE SET dkg_aggregator_state = $2",
        )
        .bind(dkg_share_id)
        .bind(Json(dkg_share_data.dkg_state))
        .execute(&self.get_conn().await?)
        .await
        .map_err(|e| DbError::BadRequest(e.to_string()))?;

        Ok(())
    }
}
