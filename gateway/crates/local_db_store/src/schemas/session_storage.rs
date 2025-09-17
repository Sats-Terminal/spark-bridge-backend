use crate::storage::LocalDbStorage;
use async_trait::async_trait;
use gateway_session_storage::traits::{RequestType, SessionRequest, SessionStatus, SessionStorage};
use persistent_storage::error::DbError;
use sqlx::types::JsonValue;
use uuid::Uuid;

#[async_trait]
impl SessionStorage for LocalDbStorage {
    async fn create_session(&self, request_type: RequestType, request_data: JsonValue) -> Result<Uuid, DbError> {
        let session_id = Uuid::new_v4();

        sqlx::query(
            r#"
            INSERT INTO gateway.session_requests
            (session_id, request_type, status, request)
            VALUES ($1, $2, $3, $4)
            "#,
        )
        .bind(session_id)
        .bind(request_type)
        .bind(SessionStatus::Pending)
        .bind(&request_data)
        .execute(&self.get_conn().await?)
        .await
        .map_err(|e| DbError::BadRequest(e.to_string()))?;

        Ok(session_id)
    }

    async fn update_session_status(&self, session_id: Uuid, status: SessionStatus) -> Result<(), DbError> {
        let rows = sqlx::query(
            r#"
            UPDATE gateway.session_requests
            SET status = $1, updated_at = now()
            WHERE session_id = $2
            "#,
        )
        .bind(status)
        .bind(session_id)
        .execute(&self.get_conn().await?)
        .await
        .map_err(|e| DbError::BadRequest(e.to_string()))?
        .rows_affected();

        if rows == 0 {
            return Err(DbError::NotFound(format!("Session {} not found", session_id)));
        }

        Ok(())
    }

    async fn set_session_error(&self, session_id: Uuid, error: &str) -> Result<(), DbError> {
        let rows = sqlx::query(
            r#"
            UPDATE gateway.session_requests
            SET status = $1, error = $2, updated_at = now()
            WHERE session_id = $3
            "#,
        )
        .bind(SessionStatus::Failed)
        .bind(error)
        .bind(session_id)
        .execute(&self.get_conn().await?)
        .await
        .map_err(|e| DbError::BadRequest(e.to_string()))?
        .rows_affected();

        if rows == 0 {
            return Err(DbError::NotFound(format!("Session {} not found", session_id)));
        }

        Ok(())
    }

    async fn set_session_success(&self, session_id: Uuid, response_data: JsonValue) -> Result<(), DbError> {
        let rows = sqlx::query(
            r#"
            UPDATE gateway.session_requests
            SET status = $1, response = $2, updated_at = now()
            WHERE session_id = $3
            "#,
        )
        .bind(SessionStatus::Success)
        .bind(&response_data)
        .bind(session_id)
        .execute(&self.get_conn().await?)
        .await
        .map_err(|e| DbError::BadRequest(e.to_string()))?
        .rows_affected();

        if rows == 0 {
            return Err(DbError::NotFound(format!("Session {} not found", session_id)));
        }

        Ok(())
    }

    async fn get_session(&self, session_id: Uuid) -> Result<SessionRequest, DbError> {
        let session = sqlx::query_as::<_, SessionRequest>(
            r#"
            SELECT * FROM gateway.session_requests
            WHERE session_id = $1
            "#,
        )
        .bind(session_id)
        .fetch_one(&self.get_conn().await?)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => DbError::NotFound(format!("Session {} not found", session_id)),
            _ => DbError::BadRequest(e.to_string()),
        })?;

        Ok(session)
    }

    async fn list_sessions(&self, limit: Option<i32>, offset: Option<i32>) -> Result<Vec<SessionRequest>, DbError> {
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
        .fetch_all(&self.get_conn().await?)
        .await
        .map_err(|e| DbError::BadRequest(e.to_string()))?;

        Ok(sessions)
    }

    async fn list_sessions_by_status(
        &self,
        status: SessionStatus,
        limit: Option<i32>,
    ) -> Result<Vec<SessionRequest>, DbError> {
        let limit = limit.unwrap_or(50);

        let sessions = sqlx::query_as::<_, SessionRequest>(
            r#"
            SELECT * FROM gateway.session_requests
            WHERE status = $1
            ORDER BY created_at DESC
            LIMIT $2
            "#,
        )
        .bind(status)
        .bind(limit)
        .fetch_all(&self.get_conn().await?)
        .await
        .map_err(|e| DbError::BadRequest(e.to_string()))?;

        Ok(sessions)
    }
}
