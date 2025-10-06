use crate::storage::LocalDbStorage;
use async_trait::async_trait;
use persistent_storage::error::DbError;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use tracing::{info, instrument};
use uuid::Uuid;

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct UserIds {
    pub user_id: Uuid,
    pub dkg_share_id: Uuid,
    pub rune_id: String,
    pub is_issuer: bool,
}

#[async_trait]
pub trait UserIdentifierStorage: Send + Sync + Debug {
    async fn get_row_by_user_id(&self, user_id: Uuid, rune_id: String) -> Result<Option<UserIds>, DbError>;
    async fn get_row_by_dkg_id(&self, dkg_share_id: Uuid) -> Result<Option<UserIds>, DbError>;
    async fn insert_user_ids(&self, user_ids: UserIds) -> Result<(), DbError>;
    async fn get_issuer_ids(&self, rune_id: String) -> Result<Option<UserIds>, DbError>;
}

#[async_trait]
impl UserIdentifierStorage for LocalDbStorage {
    #[instrument(level = "trace", skip(self), ret)]
    async fn insert_user_ids(&self, user_ids: UserIds) -> Result<(), DbError> {
        let _ = sqlx::query(
            "INSERT INTO verifier.user_identifier (user_id, dkg_share_id, rune_id, is_issuer)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (dkg_share_id) DO NOTHING",
        )
        .bind(user_ids.user_id)
        .bind(user_ids.dkg_share_id)
        .bind(user_ids.rune_id)
        .bind(user_ids.is_issuer)
        .execute(&self.get_conn().await?)
        .await
        .map_err(|e| DbError::BadRequest(e.to_string()))?;

        Ok(())
    }

    #[instrument(level = "trace", skip(self), ret)]
    async fn get_row_by_user_id(&self, user_id: Uuid, rune_id: String) -> Result<Option<UserIds>, DbError> {
        let result: Option<(Uuid, Uuid, String, bool,)> = sqlx::query_as(
            "SELECT user_id, dkg_share_id, rune_id, is_issuer FROM verifier.user_identifier WHERE user_id = $1 AND rune_id = $2;",
        )
            .bind(user_id)
            .bind(rune_id)
            .fetch_optional(&self.get_conn().await?)
            .await
            .map_err(|e| DbError::BadRequest(e.to_string()))?;
        info!("result: {result:?}");
        Ok(result.map(
            |(user_id, dkg_share_id, rune_id, is_issuer)| UserIds {
                user_id,
                dkg_share_id,
                rune_id,
                is_issuer,
            },
        ))
    }

    async fn get_row_by_dkg_id(&self, dkg_share_id: Uuid) -> Result<Option<UserIds>, DbError> {
        let result: Option<(Uuid, Uuid, String, bool,)> = sqlx::query_as(
            "SELECT user_id, dkg_share_id, rune_id, is_issuer FROM verifier.user_identifier WHERE dkg_share_id = $1;",
        )
            .bind(dkg_share_id)
            .fetch_optional(&self.get_conn().await?)
            .await
            .map_err(|e| DbError::BadRequest(e.to_string()))?;

        Ok(result.map(
            |(user_id, dkg_share_id, rune_id, is_issuer)| UserIds {
                user_id,
                dkg_share_id,
                rune_id,
                is_issuer,
            },
        ))
    }

    #[instrument(level = "trace", skip(self))]
    async fn get_issuer_ids(&self, rune_id: String) -> Result<Option<UserIds>, DbError> {
        let result: Option<(Uuid, Uuid, String, bool)> = sqlx::query_as(
            "SELECT user_id, dkg_share_id, rune_id, is_issuer
            FROM verifier.user_identifier
            WHERE is_issuer = true AND rune_id = $1",
        )
        .bind(rune_id)
        .fetch_optional(&self.get_conn().await?)
        .await
        .map_err(|e| DbError::BadRequest(e.to_string()))?;

        Ok(result.map(|(user_id, dkg_share_id, rune_id, is_issuer)| UserIds {
            user_id,
            dkg_share_id,
            rune_id,
            is_issuer,
        }))
    }
}
