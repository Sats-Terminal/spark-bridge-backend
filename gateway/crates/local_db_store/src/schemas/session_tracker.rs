use async_trait::async_trait;
use chrono::NaiveDateTime;
use crate::storage::Storage;
use persistent_storage::error::DatabaseError;
use persistent_storage::init::PostgresRepo;
use serde_json::Value as JsonValue;
use sqlx::FromRow;
use std::fmt::Display;
use uuid::Uuid;

#[derive(Debug, Clone, FromRow)]
pub struct SessionRequest {
    pub session_id: Uuid,
    pub request_type: String,
    pub status: String,
    pub request: JsonValue,
    pub response: Option<JsonValue>,
    pub error: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Clone)]
pub enum RequestType {
    SendRunes,
    CreateTransaction,
    BroadcastTransaction,
    GenerateFrostSignature,
    Other(String),
}

impl Display for RequestType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            RequestType::SendRunes => "send_runes".to_string(),
            RequestType::CreateTransaction => "create_transaction".to_string(),
            RequestType::BroadcastTransaction => "broadcast_transaction".to_string(),
            RequestType::GenerateFrostSignature => "generate_frost_signature".to_string(),
            RequestType::Other(s) => s.clone(),
        };
        write!(f, "{}", str)
    }
}

impl From<String> for RequestType {
    fn from(s: String) -> Self {
        match s.as_str() {
            "send_runes" => RequestType::SendRunes,
            "create_transaction" => RequestType::CreateTransaction,
            "broadcast_transaction" => RequestType::BroadcastTransaction,
            "generate_frost_signature" => RequestType::GenerateFrostSignature,
            _ => RequestType::Other(s),
        }
    }
}

#[derive(Debug, Clone)]
pub enum SessionStatus {
    Pending,
    InProgress,
    Success,
    Failed,
}

impl Display for SessionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            SessionStatus::Pending => "pending".to_string(),
            SessionStatus::InProgress => "in_progress".to_string(),
            SessionStatus::Success => "success".to_string(),
            SessionStatus::Failed => "failed".to_string(),
        };
        write!(f, "{}", str)
    }
}

impl From<String> for SessionStatus {
    fn from(s: String) -> Self {
        match s.as_str() {
            "pending" => SessionStatus::Pending,
            "in_progress" => SessionStatus::InProgress,
            "success" => SessionStatus::Success,
            "failed" => SessionStatus::Failed,
            _ => SessionStatus::Pending,
        }
    }
}

#[async_trait]
pub trait SessionStorage {
    async fn create_session(&self, request_type: RequestType, request_data: JsonValue) -> Result<Uuid, DatabaseError>;

    async fn update_session_status(&self, session_id: Uuid, status: SessionStatus) -> Result<(), DatabaseError>;

    async fn set_session_error(&self, session_id: Uuid, error: &str) -> Result<(), DatabaseError>;

    async fn set_session_success(&self, session_id: Uuid, response_data: JsonValue) -> Result<(), DatabaseError>;

    async fn get_session(&self, session_id: Uuid) -> Result<SessionRequest, DatabaseError>;

    async fn list_sessions(
        &self,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<Vec<SessionRequest>, DatabaseError>;

    async fn list_sessions_by_status(
        &self,
        status: SessionStatus,
        limit: Option<i32>,
    ) -> Result<Vec<SessionRequest>, DatabaseError>;
}

#[async_trait]
impl SessionStorage for PostgresRepo {
    async fn create_session(&self, request_type: RequestType, request_data: JsonValue) -> Result<Uuid, DatabaseError> {
        let session_id = Uuid::new_v4();

        sqlx::query(
            r#"
            INSERT INTO gateway.session_requests
            (session_id, request_type, status, request)
            VALUES ($1, $2, $3, $4)
            "#,
        )
        .bind(session_id)
        .bind(request_type.to_string())
        .bind(SessionStatus::Pending.to_string())
        .bind(&request_data)
        .execute(&self.pool)
        .await
        .map_err(|e| DatabaseError::BadRequest(e.to_string()))?;

        Ok(session_id)
    }

    async fn update_session_status(&self, session_id: Uuid, status: SessionStatus) -> Result<(), DatabaseError> {
        let rows = sqlx::query(
            r#"
            UPDATE gateway.session_requests
            SET status = $1, updated_at = now()
            WHERE session_id = $2
            "#,
        )
        .bind(status.to_string())
        .bind(session_id)
        .execute(&self.pool)
        .await
        .map_err(|e| DatabaseError::BadRequest(e.to_string()))?
        .rows_affected();

        if rows == 0 {
            return Err(DatabaseError::NotFound(format!("Session {} not found", session_id)));
        }

        Ok(())
    }

    async fn set_session_error(&self, session_id: Uuid, error: &str) -> Result<(), DatabaseError> {
        let rows = sqlx::query(
            r#"
            UPDATE gateway.session_requests
            SET status = $1, error = $2, updated_at = now()
            WHERE session_id = $3
            "#,
        )
        .bind(SessionStatus::Failed.to_string())
        .bind(error)
        .bind(session_id)
        .execute(&self.pool)
        .await
        .map_err(|e| DatabaseError::BadRequest(e.to_string()))?
        .rows_affected();

        if rows == 0 {
            return Err(DatabaseError::NotFound(format!("Session {} not found", session_id)));
        }

        Ok(())
    }

    async fn set_session_success(&self, session_id: Uuid, response_data: JsonValue) -> Result<(), DatabaseError> {
        let rows = sqlx::query(
            r#"
            UPDATE gateway.session_requests
            SET status = $1, response = $2, updated_at = now()
            WHERE session_id = $3
            "#,
        )
        .bind(SessionStatus::Success.to_string())
        .bind(&response_data)
        .bind(session_id)
        .execute(&self.pool)
        .await
        .map_err(|e| DatabaseError::BadRequest(e.to_string()))?
        .rows_affected();

        if rows == 0 {
            return Err(DatabaseError::NotFound(format!("Session {} not found", session_id)));
        }

        Ok(())
    }

    async fn get_session(&self, session_id: Uuid) -> Result<SessionRequest, DatabaseError> {
        let session = sqlx::query_as::<_, SessionRequest>(
            r#"
            SELECT * FROM gateway.session_requests
            WHERE session_id = $1
            "#,
        )
        .bind(session_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => DatabaseError::NotFound(format!("Session {} not found", session_id)),
            _ => DatabaseError::BadRequest(e.to_string()),
        })?;

        Ok(session)
    }

    async fn list_sessions(
        &self,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<Vec<SessionRequest>, DatabaseError> {
        let limit = limit.unwrap_or(50);
        let offset = offset.unwrap_or(0);

        let sessions = sqlx::query_as::<_, SessionRequest>(
            r#"
            SELECT * FROM gateway.session_requests
            ORDER BY created_at DESC
            LIMIT $1 OFFSET $2
            "#,
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DatabaseError::BadRequest(e.to_string()))?;

        Ok(sessions)
    }

    async fn list_sessions_by_status(
        &self,
        status: SessionStatus,
        limit: Option<i32>,
    ) -> Result<Vec<SessionRequest>, DatabaseError> {
        let limit = limit.unwrap_or(50);

        let sessions = sqlx::query_as::<_, SessionRequest>(
            r#"
            SELECT * FROM gateway.session_requests
            WHERE status = $1
            ORDER BY created_at DESC
            LIMIT $2
            "#,
        )
        .bind(status.to_string())
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DatabaseError::BadRequest(e.to_string()))?;

        Ok(sessions)
    }
}

pub struct SessionTracker<'a> {
    pub repo: &'a Storage,
}

impl<'a> SessionTracker<'a> {
    pub fn new(repo: &'a Storage) -> Self {
        Self { repo }
    }

    pub async fn start_session(
        &self,
        request_type: RequestType,
        request_data: JsonValue,
    ) -> Result<Uuid, DatabaseError> {
        let session_id = self
            .repo
            .postgres_repo
            .create_session(request_type, request_data)
            .await?;
        self.repo
            .postgres_repo
            .update_session_status(session_id, SessionStatus::InProgress)
            .await?;
        Ok(session_id)
    }

    pub async fn complete_session(&self, session_id: Uuid, response_data: JsonValue) -> Result<(), DatabaseError> {
        self.repo
            .postgres_repo
            .set_session_success(session_id, response_data)
            .await
    }

    pub async fn fail_session(&self, session_id: Uuid, error: &str) -> Result<(), DatabaseError> {
        self.repo.postgres_repo.set_session_error(session_id, error).await
    }

    pub async fn get_session_status(&self, session_id: Uuid) -> Result<SessionStatus, DatabaseError> {
        let session = self.repo.postgres_repo.get_session(session_id).await?;
        Ok(SessionStatus::from(session.status))
    }
}
