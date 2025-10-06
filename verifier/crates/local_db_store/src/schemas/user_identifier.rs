use crate::storage::LocalDbStorage;
use async_trait::async_trait;
use frost::types::DkgShareId;
use persistent_storage::error::DbError;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use tracing::{info, instrument};
use uuid::Uuid;

pub type UserUuid = Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserUniqueId {
    pub uuid: UserUuid,
    pub rune_id: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct UserIdentifier {
    pub user_uuid: Uuid,
    pub dkg_share_id: Uuid,
    pub public_key: String,
    pub rune_id: String,
    pub is_issuer: bool,
}

#[derive(Debug, Clone)]
pub struct UserIdentifierData {
    pub public_key: String,
    pub rune_id: String,
    pub is_issuer: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserIds {
    pub user_uuid: Uuid,
    pub dkg_share_id: Uuid,
    pub rune_id: String,
}

#[async_trait]
pub trait UserIdentifierStorage: Send + Sync + Debug {
    async fn get_row_by_user_unique_id(&self, dkg_share_id: &UserUniqueId) -> Result<Option<UserIdentifier>, DbError>;
    async fn get_dkg_share_data_via_dkg_share(
        &self,
        dkg_share_id: &DkgShareId,
    ) -> Result<Option<UserIdentifier>, DbError>;
    async fn set_user_identifier_data(
        &self,
        user_identifier: &UserUuid,
        dkg_share_id: &DkgShareId,
        user_identifier_data: UserIdentifierData,
    ) -> Result<(), DbError>;
    async fn get_issuer_ids(&self, rune_id: String) -> Result<Option<UserIds>, DbError>;
}

#[async_trait]
impl UserIdentifierStorage for LocalDbStorage {
    #[instrument(level = "trace", skip(self), ret)]
    async fn set_user_identifier_data(
        &self,
        user_identifier: &UserUuid,
        dkg_share_id: &DkgShareId,
        user_identifier_data: UserIdentifierData,
    ) -> Result<(), DbError> {
        let _ = sqlx::query(
            "INSERT INTO verifier.user_identifier (user_uuid, dkg_share_id, public_key, rune_id, is_issuer)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (user_uuid, rune_id) DO NOTHING",
        )
        .bind(user_identifier)
        .bind(dkg_share_id)
        .bind(user_identifier_data.public_key)
        .bind(user_identifier_data.rune_id)
        .bind(user_identifier_data.is_issuer)
        .execute(&self.get_conn().await?)
        .await
        .map_err(|e| DbError::BadRequest(e.to_string()))?;

        Ok(())
    }

    #[instrument(level = "trace", skip(self), ret)]
    async fn get_row_by_user_unique_id(&self, id: &UserUniqueId) -> Result<Option<UserIdentifier>, DbError> {
        let result: Option<(Uuid, Uuid, String, String, bool,)> = sqlx::query_as(
            "SELECT user_uuid, dkg_share_id, public_key, rune_id, is_issuer FROM verifier.user_identifier WHERE user_uuid = $1 AND rune_id = $2;",
        )
            .bind(id.uuid)
            .bind(&id.rune_id)
            .fetch_optional(&self.get_conn().await?)
            .await
            .map_err(|e| DbError::BadRequest(e.to_string()))?;
        info!("result: {result:?}");
        Ok(result.map(
            |(user_uuid, dkg_share_id, public_key, rune_id, is_issuer)| UserIdentifier {
                user_uuid,
                dkg_share_id,
                public_key,
                rune_id,
                is_issuer,
            },
        ))
    }

    async fn get_dkg_share_data_via_dkg_share(
        &self,
        dkg_share_id: &DkgShareId,
    ) -> Result<Option<UserIdentifier>, DbError> {
        let result: Option<(Uuid, Uuid, String, String, bool,)> = sqlx::query_as(
            "SELECT user_uuid, dkg_share_id, public_key, rune_id, is_issuer FROM verifier.user_identifier WHERE dkg_share_id = $1;",
        )
            .bind(dkg_share_id)
            .fetch_optional(&self.get_conn().await?)
            .await
            .map_err(|e| DbError::BadRequest(e.to_string()))?;

        Ok(result.map(
            |(user_uuid, dkg_share_id, public_key, rune_id, is_issuer)| UserIdentifier {
                user_uuid,
                dkg_share_id,
                public_key,
                rune_id,
                is_issuer,
            },
        ))
    }

    #[instrument(level = "trace", skip(self))]
    async fn get_issuer_ids(&self, rune_id: String) -> Result<Option<UserIds>, DbError> {
        let result: Option<(UserUuid, DkgShareId, String)> = sqlx::query_as(
            "SELECT (user_uuid, dkg_share_id, rune_id)
            FROM verifier.user_identifier
            WHERE is_issuer = true AND rune_id = $1",
        )
        .bind(rune_id)
        .fetch_optional(&self.get_conn().await?)
        .await
        .map_err(|e| DbError::BadRequest(e.to_string()))?;

        Ok(result.map(|(user_uuid, dkg_share_id, rune_id)| UserIds {
            user_uuid,
            dkg_share_id,
            rune_id,
        }))
    }
}
