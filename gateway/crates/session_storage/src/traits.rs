use async_trait::async_trait;
use chrono::NaiveDateTime;
use persistent_storage::error::DbError;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::FromRow;
use std::fmt::Display;
use uuid::Uuid;

#[derive(Debug, Clone, FromRow)]
pub struct SessionRequest {
    pub session_id: Uuid,
    pub request_type: String,
    pub status: SessionStatus,
    pub request: JsonValue,
    pub response: Option<JsonValue>,
    pub error: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Serialize, Deserialize, sqlx::Type, Clone, Copy, Eq, PartialEq, Hash)]
#[sqlx(rename_all = "snake_case", type_name = "REQ_TYPE")]
pub enum RequestType {
    SendRunes,
    CreateTransaction,
    BroadcastTransaction,
    GenerateFrostSignature,
}

#[derive(Debug, Serialize, Deserialize, sqlx::Type, Clone, Copy, Eq, PartialEq, Hash)]
#[sqlx(rename_all = "snake_case", type_name = "SESSION_STATUS")]
pub enum SessionStatus {
    Pending,
    InProgress,
    Success,
    Failed,
}

#[async_trait]
pub trait SessionStorage {
    async fn create_session(&self, request_type: RequestType, request_data: JsonValue) -> Result<Uuid, DbError>;

    async fn update_session_status(&self, session_id: Uuid, status: SessionStatus) -> Result<(), DbError>;

    async fn set_session_error(&self, session_id: Uuid, error: &str) -> Result<(), DbError>;

    async fn set_session_success(&self, session_id: Uuid, response_data: JsonValue) -> Result<(), DbError>;

    async fn get_session(&self, session_id: Uuid) -> Result<SessionRequest, DbError>;

    async fn list_sessions(&self, limit: Option<i32>, offset: Option<i32>) -> Result<Vec<SessionRequest>, DbError>;

    async fn list_sessions_by_status(
        &self,
        status: SessionStatus,
        limit: Option<i32>,
    ) -> Result<Vec<SessionRequest>, DbError>;
}
