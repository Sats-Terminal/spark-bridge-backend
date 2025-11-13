use crate::storage::LocalDbStorage;
use async_trait::async_trait;
use persistent_storage::error::DbError;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use tracing::instrument;
use uuid::Uuid;
use bitcoin::secp256k1::PublicKey;
use bitcoin::secp256k1::XOnlyPublicKey;
use std::str::FromStr;

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum UserId {
    Uuid(Uuid),
    PublicKey(PublicKey),
    XOnlyPublicKey(XOnlyPublicKey),
}

impl ToString for UserId {
    fn to_string(&self) -> String {
        match self {
            UserId::Uuid(uuid) => uuid.to_string(),
            UserId::PublicKey(public_key) => public_key.to_string(),
            UserId::XOnlyPublicKey(x_only_public_key) => x_only_public_key.to_string(),
        }
    }
}

impl FromStr for UserId {
    type Err = DbError;
    fn from_str(s: &str) -> Result<Self, DbError> {
        let uuid_response = Uuid::from_str(s);
        if uuid_response.is_ok() {
            return Ok(UserId::Uuid(uuid_response.unwrap()));
        }
        let public_key_response = PublicKey::from_str(s);
        if public_key_response.is_ok() {
            return Ok(UserId::PublicKey(public_key_response.unwrap()));
        }
        let x_only_public_key_response = XOnlyPublicKey::from_str(s);
        if x_only_public_key_response.is_ok() {
            return Ok(UserId::XOnlyPublicKey(x_only_public_key_response.unwrap()));
        }
        Err(DbError::DecodeError(format!("Invalid user id, it is not uuid, public key or x only public key: {}", s)))
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct UserIds {
    pub user_id: UserId,
    pub dkg_share_id: Uuid,
    pub rune_id: String,
    pub is_issuer: bool,
}

#[async_trait]
pub trait UserIdentifierStorage: Send + Sync + Debug {
    async fn get_row_by_user_id(&self, user_id: UserId, rune_id: String) -> Result<Option<UserIds>, DbError>;
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
        .bind(user_ids.user_id.to_string())
        .bind(user_ids.dkg_share_id)
        .bind(user_ids.rune_id)
        .bind(user_ids.is_issuer)
        .execute(&self.get_conn().await?)
        .await
        .map_err(|e| DbError::BadRequest(e.to_string()))?;

        Ok(())
    }

    #[instrument(level = "trace", skip(self), ret)]
    async fn get_row_by_user_id(&self, user_id: UserId, rune_id: String) -> Result<Option<UserIds>, DbError> {
        let result: Option<(String, Uuid, String, bool,)> = sqlx::query_as(
            "SELECT user_id, dkg_share_id, rune_id, is_issuer FROM verifier.user_identifier WHERE user_id = $1 AND rune_id = $2;",
        )
            .bind(user_id.to_string())
            .bind(rune_id)
            .fetch_optional(&self.get_conn().await?)
            .await
            .map_err(|e| DbError::BadRequest(e.to_string()))?;
        
        match result {
            Some((user_id, dkg_share_id, rune_id, is_issuer)) => Ok(Some(UserIds {
                user_id: UserId::from_str(&user_id)?,
                dkg_share_id,
                rune_id,
                is_issuer,
            })),
            None => Ok(None),
        }
    }

    async fn get_row_by_dkg_id(&self, dkg_share_id: Uuid) -> Result<Option<UserIds>, DbError> {
        let result: Option<(String, Uuid, String, bool)> = sqlx::query_as(
            "SELECT user_id, dkg_share_id, rune_id, is_issuer FROM verifier.user_identifier WHERE dkg_share_id = $1;",
        )
        .bind(dkg_share_id)
        .fetch_optional(&self.get_conn().await?)
        .await
        .map_err(|e| DbError::BadRequest(e.to_string()))?;

        match result {
            Some((user_id, dkg_share_id, rune_id, is_issuer)) => Ok(Some(UserIds {
                user_id: UserId::from_str(&user_id)?,
                dkg_share_id,
                rune_id,
                is_issuer,
            })),
            None => Ok(None),
        }
    }

    #[instrument(level = "trace", skip(self))]
    async fn get_issuer_ids(&self, rune_id: String) -> Result<Option<UserIds>, DbError> {
        let result: Option<(String, Uuid, String, bool)> = sqlx::query_as(
            "SELECT user_id, dkg_share_id, rune_id, is_issuer
            FROM verifier.user_identifier
            WHERE is_issuer = true AND rune_id = $1",
        )
        .bind(rune_id)
        .fetch_optional(&self.get_conn().await?)
        .await
        .map_err(|e| DbError::BadRequest(e.to_string()))?;

        match result {
            Some((user_id, dkg_share_id, rune_id, is_issuer)) => Ok(Some(UserIds {
                user_id: UserId::from_str(&user_id)?,
                dkg_share_id,
                rune_id,
                is_issuer,
            })),
            None => Ok(None),
        }
    }
}
