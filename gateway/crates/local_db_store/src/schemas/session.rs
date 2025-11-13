use async_trait::async_trait;
use persistent_storage::error::DbError;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, types::Json};
use tracing::instrument;
use uuid::Uuid;

use crate::storage::LocalDbStorage;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub request_id: Uuid,
    pub request_type: RequestType,
    pub request_status: RequestStatus,
    pub deposit_address: String,
    pub error_details: Option<RequestErrorDetails>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RequestErrorDetails {
    Timeout(String),
    InvalidData(String),
}

#[derive(Debug, Clone, FromRow)]
struct RequestRow {
    pub request_id: Uuid,
    pub request_type: RequestType,
    pub request_status: RequestStatus,
    pub deposit_address: String,
    pub error_details: Option<Json<RequestErrorDetails>>,
}

impl From<RequestRow> for SessionInfo {
    fn from(row: RequestRow) -> Self {
        Self {
            request_id: row.request_id,
            request_type: row.request_type,
            request_status: row.request_status,
            deposit_address: row.deposit_address,
            error_details: row.error_details.map(|error_details| error_details.0),
        }
    }
}

impl From<SessionInfo> for RequestRow {
    fn from(val: SessionInfo) -> Self {
        RequestRow {
            request_id: val.request_id,
            request_type: val.request_type,
            request_status: val.request_status,
            deposit_address: val.deposit_address,
            error_details: val.error_details.map(Json),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, sqlx::Type, Eq, PartialEq)]
#[sqlx(type_name = "REQUEST_TYPE", rename_all = "snake_case")]
pub enum RequestType {
    BridgeRunes,
    ExitSpark,
}

#[derive(Debug, Serialize, Deserialize, sqlx::Type, Clone, Copy, Eq, PartialEq, Hash)]
#[sqlx(rename_all = "snake_case", type_name = "REQUEST_STATUS")]
pub enum RequestStatus {
    Pending,
    Completed,
    Failed,
}

#[async_trait]
pub trait SessionStorage: Send + Sync {
    async fn insert_session(&self, session_info: SessionInfo) -> Result<(), DbError>;
    async fn update_session_status(
        &self,
        request_id: Uuid,
        status: RequestStatus,
        error_details: Option<RequestErrorDetails>,
    ) -> Result<(), DbError>;
    async fn get_session(&self, request_id: Uuid) -> Result<SessionInfo, DbError>;
}

#[async_trait]
impl SessionStorage for LocalDbStorage {
    #[instrument(level = "trace", skip_all)]
    async fn insert_session(&self, session_info: SessionInfo) -> Result<(), DbError> {
        let session_info: RequestRow = session_info.into();
        let _ = sqlx::query(
            r#"
            INSERT INTO gateway.sessions (request_id, request_type, request_status, deposit_address, error_details)
            VALUES ($1, $2, $3, $4)
        "#,
        )
        .bind(session_info.request_id)
        .bind(session_info.request_type)
        .bind(session_info.request_status)
        .bind(session_info.deposit_address)
        .bind(session_info.error_details)
        .execute(&self.get_conn().await?)
        .await
        .map_err(|e| DbError::BadRequest(e.to_string()))?;
        Ok(())
    }

    #[instrument(level = "trace", skip_all)]
    async fn update_session_status(
        &self,
        request_id: Uuid,
        status: RequestStatus,
        error_details: Option<RequestErrorDetails>,
    ) -> Result<(), DbError> {
        let query = r#"
            UPDATE gateway.sessions
            SET request_status = $1, error_details = $2
            WHERE request_id = $2
        "#;
        let _ = sqlx::query(query)
            .bind(status)
            .bind(request_id)
            .bind(Json(error_details))
            .execute(&self.get_conn().await?)
            .await
            .map_err(|e| DbError::BadRequest(e.to_string()))?;
        Ok(())
    }

    #[instrument(level = "trace", skip_all)]
    async fn get_session(&self, session_id: Uuid) -> Result<SessionInfo, DbError> {
        let query = r#"
            SELECT request_id, request_type, request_status, deposit_address, error_details
            FROM gateway.sessions
            WHERE request_id = $1
        "#;
        let row: RequestRow = sqlx::query_as(query)
            .bind(session_id)
            .fetch_one(&self.get_conn().await?)
            .await
            .map_err(|e| DbError::BadRequest(e.to_string()))?;

        Ok(SessionInfo::from(row))
    }
}

#[cfg(test)]
mod tests {
    use persistent_storage::error::DbError as DatabaseError;

    use super::*;
    use crate::storage::make_repo_with_config;
}
