use crate::traits::{RequestType, SessionStatus, SessionStorage};
use persistent_storage::error::DbError;
use sqlx::types::JsonValue;
use std::sync::Arc;
use uuid::Uuid;

pub struct SessionTracker {
    pub repo: Arc<dyn SessionStorage>,
}

impl SessionTracker {
    pub async fn start_session(&self, request_type: RequestType, request_data: JsonValue) -> Result<Uuid, DbError> {
        let session_id = self.repo.create_session(request_type, request_data).await?;
        self.repo
            .update_session_status(session_id, SessionStatus::InProgress)
            .await?;
        Ok(session_id)
    }

    pub async fn complete_session(&self, session_id: Uuid, response_data: JsonValue) -> Result<(), DbError> {
        self.repo.set_session_success(session_id, response_data).await
    }

    pub async fn fail_session(&self, session_id: Uuid, error: &str) -> Result<(), DbError> {
        self.repo.set_session_error(session_id, error).await
    }

    pub async fn get_session_status(&self, session_id: Uuid) -> Result<SessionStatus, DbError> {
        let session = self.repo.get_session(session_id).await?;
        Ok(session.status)
    }
}
