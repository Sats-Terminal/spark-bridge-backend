use crate::storage::LocalDbStorage;
use async_trait::async_trait;
use global_utils::common_types::get_uuid;
use persistent_storage::error::DbError;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use sqlx::types::Json;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SessionInfo {
    pub request_type: RequestType,
    pub request_status: SessionStatus,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestType {
    GetRunesDepositAddress,
    GetSparkDepositAddress,
    BridgeRunes,
    ExitSpark,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Pending(String),
    Success,
    Failed(String),
}

#[async_trait]
pub trait SessionStorage {
    async fn create_session(&self, session_info: SessionInfo) -> Result<Uuid, DbError>;
    async fn update_session_status(&self, session_id: Uuid, status: SessionStatus) -> Result<(), DbError>;
}


#[async_trait]
impl SessionStorage for LocalDbStorage {
    async fn create_session(&self, session_info: SessionInfo) -> Result<Uuid, DbError> {
        let session_id = get_uuid();
        let query = "INSERT INTO gateway.session_info (session_id, request_type, status) VALUES ($1, $2, $3)";
        sqlx::query(query)
            .bind(session_id)
            .bind(Json(session_info.request_type))
            .bind(Json(session_info.request_status))
            .execute(&self.get_conn().await?)
            .await
            .map_err(|e| DbError::BadRequest(e.to_string()))?;
        Ok(session_id)
    }

    async fn update_session_status(&self, session_id: Uuid, status: SessionStatus) -> Result<(), DbError> {
        let query = "UPDATE gateway.session_info SET status = $1 WHERE session_id = $2";
        sqlx::query(query)
            .bind(Json(status))
            .bind(session_id)
            .execute(&self.get_conn().await?)
            .await
            .map_err(|e| DbError::BadRequest(e.to_string()))?;
        Ok(())
    }
}
