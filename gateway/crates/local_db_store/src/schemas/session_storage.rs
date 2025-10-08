use crate::storage::LocalDbStorage;
use async_trait::async_trait;
use global_utils::common_types::get_uuid;
use persistent_storage::error::DbError;
use persistent_storage::init::StorageHealthcheck;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use sqlx::types::Json;
use tracing::instrument;
use uuid::Uuid;

pub type SessionUuid = Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub request_type: RequestType,
    pub request_status: Json<SessionStatus>,
}

#[derive(Debug, Clone, FromRow)]
struct SessionInfoRow {
    pub request_type: RequestType,
    pub request_status: SessionStatus,
}

impl From<SessionInfoRow> for SessionInfo {
    fn from(row: SessionInfoRow) -> Self {
        Self {
            request_type: row.request_type,
            request_status: Json(row.request_status),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, sqlx::Type, Eq, PartialEq)]
#[sqlx(type_name = "REQ_TYPE", rename_all = "snake_case")]
pub enum RequestType {
    GetRunesDepositAddress,
    GetSparkDepositAddress,
    BridgeRunes,
    ExitSpark,
}

#[derive(Debug, Serialize, Deserialize, sqlx::Type, Clone, Copy, Eq, PartialEq, Hash)]
#[sqlx(rename_all = "snake_case", type_name = "REQUEST_STATUS")]
pub enum SessionStatus {
    Pending,
    Processing,
    Completed,
    Failed,
    Cancelled,
}

#[async_trait]
pub trait SessionStorage: Send + Sync + StorageHealthcheck {
    async fn create_session(&self, session_info: SessionInfo) -> Result<SessionUuid, DbError>;
    async fn update_session_status(&self, session_id: SessionUuid, status: SessionStatus) -> Result<(), DbError>;
    async fn get_session(&self, session_id: SessionUuid) -> Result<SessionInfo, DbError>;
}

#[async_trait]
impl SessionStorage for LocalDbStorage {
    #[instrument(level = "trace", skip_all)]
    async fn create_session(&self, session_info: SessionInfo) -> Result<SessionUuid, DbError> {
        let session_id = get_uuid();
        let query = r#"
            INSERT INTO gateway.session_requests (session_id, request_type, request_status)
            VALUES ($1, $2, $3)
        "#;
        sqlx::query(query)
            .bind(session_id)
            .bind(session_info.request_type)
            .bind(session_info.request_status.0)
            .execute(&self.get_conn().await?)
            .await
            .map_err(|e| DbError::BadRequest(e.to_string()))?;
        Ok(session_id)
    }

    #[instrument(level = "trace", skip_all)]
    async fn update_session_status(&self, session_id: SessionUuid, status: SessionStatus) -> Result<(), DbError> {
        let query = r#"
            UPDATE gateway.session_requests
            SET request_status = $1
            WHERE session_id = $2
        "#;
        sqlx::query(query)
            .bind(status)
            .bind(session_id)
            .execute(&self.get_conn().await?)
            .await
            .map_err(|e| DbError::BadRequest(e.to_string()))?;
        Ok(())
    }

    #[instrument(level = "trace", skip_all)]
    async fn get_session(&self, session_id: SessionUuid) -> Result<SessionInfo, DbError> {
        let query = r#"
            SELECT request_type, request_status
            FROM gateway.session_requests
            WHERE session_id = $1
        "#;
        let row: SessionInfoRow = sqlx::query_as(query)
            .bind(session_id)
            .fetch_one(&self.get_conn().await?)
            .await
            .map_err(|e| DbError::BadRequest(e.to_string()))?;

        Ok(SessionInfo::from(row))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::make_repo_with_config;
    use persistent_storage::error::DbError as DatabaseError;

    pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

    async fn cleanup_and_setup(repo: &LocalDbStorage) {
        sqlx::query("TRUNCATE gateway.session_requests RESTART IDENTITY CASCADE")
            .execute(&repo.postgres_repo.pool)
            .await
            .unwrap();
    }

    fn create_test_session() -> SessionInfo {
        SessionInfo {
            request_type: RequestType::GetRunesDepositAddress,
            request_status: Json(SessionStatus::Pending),
        }
    }

    fn create_test_session_with_type(request_type: RequestType, status: SessionStatus) -> SessionInfo {
        SessionInfo {
            request_type,
            request_status: Json(status),
        }
    }

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn test_create_session(db: sqlx::PgPool) -> Result<(), DatabaseError> {
        let repo = make_repo_with_config(db).await;
        cleanup_and_setup(&repo).await;

        let session = create_test_session();
        let session_id = repo.create_session(session.clone()).await?;

        let row: (SessionStatus,) =
            sqlx::query_as("SELECT request_status FROM gateway.session_requests WHERE session_id = $1")
                .bind(session_id)
                .fetch_one(&repo.postgres_repo.pool)
                .await?;

        assert_eq!(row.0, *session.request_status);
        Ok(())
    }

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn test_update_session_status(db: sqlx::PgPool) -> Result<(), DatabaseError> {
        let repo = make_repo_with_config(db).await;
        cleanup_and_setup(&repo).await;

        let session = create_test_session();
        let session_id = repo.create_session(session).await?;

        repo.update_session_status(session_id, SessionStatus::Completed).await?;

        let row: (SessionStatus,) =
            sqlx::query_as("SELECT request_status FROM gateway.session_requests WHERE session_id = $1")
                .bind(session_id)
                .fetch_one(&repo.postgres_repo.pool)
                .await?;

        assert_eq!(row.0, SessionStatus::Completed);
        Ok(())
    }

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn test_get_session(db: sqlx::PgPool) -> Result<(), DatabaseError> {
        let repo = make_repo_with_config(db).await;
        cleanup_and_setup(&repo).await;

        let original_session = create_test_session_with_type(RequestType::BridgeRunes, SessionStatus::Processing);
        let session_id = repo.create_session(original_session.clone()).await?;

        let retrieved_session = repo.get_session(session_id).await?;

        assert_eq!(retrieved_session.request_type, original_session.request_type);
        assert_eq!(*retrieved_session.request_status, *original_session.request_status);
        Ok(())
    }

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn test_get_nonexistent_session(db: sqlx::PgPool) -> Result<(), DatabaseError> {
        let repo = make_repo_with_config(db).await;
        cleanup_and_setup(&repo).await;

        let fake_id = get_uuid();
        let result = repo.get_session(fake_id).await;

        assert!(result.is_err());
        Ok(())
    }

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn test_update_nonexistent_session(db: sqlx::PgPool) -> Result<(), DatabaseError> {
        let repo = make_repo_with_config(db).await;
        cleanup_and_setup(&repo).await;

        let fake_id = get_uuid();
        let result = repo.update_session_status(fake_id, SessionStatus::Failed).await;

        assert!(result.is_ok());
        Ok(())
    }

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn test_create_different_session_types(db: sqlx::PgPool) -> Result<(), DatabaseError> {
        let repo = make_repo_with_config(db).await;
        cleanup_and_setup(&repo).await;

        let session_types = vec![
            RequestType::GetRunesDepositAddress,
            RequestType::GetSparkDepositAddress,
            RequestType::BridgeRunes,
            RequestType::ExitSpark,
        ];

        for session_type in session_types {
            let session = create_test_session_with_type(session_type.clone(), SessionStatus::Pending);
            let session_id = repo.create_session(session).await?;

            let row: (RequestType,) =
                sqlx::query_as("SELECT request_type FROM gateway.session_requests WHERE session_id = $1")
                    .bind(session_id)
                    .fetch_one(&repo.postgres_repo.pool)
                    .await?;

            assert_eq!(row.0, session_type);
        }
        Ok(())
    }

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn test_all_session_statuses(db: sqlx::PgPool) -> Result<(), DatabaseError> {
        let repo = make_repo_with_config(db).await;
        cleanup_and_setup(&repo).await;

        let statuses = vec![
            SessionStatus::Pending,
            SessionStatus::Processing,
            SessionStatus::Completed,
            SessionStatus::Failed,
            SessionStatus::Cancelled,
        ];

        for status in statuses {
            let session = create_test_session_with_type(RequestType::GetRunesDepositAddress, status);
            let session_id = repo.create_session(session).await?;

            let retrieved_session = repo.get_session(session_id).await?;
            assert_eq!(*retrieved_session.request_status, status);
        }
        Ok(())
    }

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn test_status_progression(db: sqlx::PgPool) -> Result<(), DatabaseError> {
        let repo = make_repo_with_config(db).await;
        cleanup_and_setup(&repo).await;

        let session = create_test_session();
        let session_id = repo.create_session(session).await?;

        let status_progression = vec![SessionStatus::Processing, SessionStatus::Completed];

        for status in status_progression {
            repo.update_session_status(session_id, status).await?;
            let updated_session = repo.get_session(session_id).await?;
            assert_eq!(*updated_session.request_status, status);
        }
        Ok(())
    }

    const ITERATIONS: usize = 5;
    const ITERATIONS_V2: usize = 100;

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn test_create_multiple_sessions(db: sqlx::PgPool) -> Result<(), DatabaseError> {
        let repo = make_repo_with_config(db).await;
        cleanup_and_setup(&repo).await;

        let mut session_ids = Vec::new();
        for _i in 0..ITERATIONS {
            let session = create_test_session_with_type(RequestType::GetRunesDepositAddress, SessionStatus::Pending);
            let session_id = repo.create_session(session).await?;
            session_ids.push(session_id);
        }

        assert_eq!(session_ids.len(), 5);
        for session_id in session_ids {
            let session = repo.get_session(session_id).await?;
            assert_eq!(*session.request_status, SessionStatus::Pending);
        }
        Ok(())
    }

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn test_session_serialization(db: sqlx::PgPool) -> Result<(), DatabaseError> {
        let repo = make_repo_with_config(db).await;
        cleanup_and_setup(&repo).await;

        let session = create_test_session_with_type(RequestType::BridgeRunes, SessionStatus::Processing);

        let json_str = serde_json::to_string(&session).unwrap();
        let deserialized: SessionInfo = serde_json::from_str(&json_str).unwrap();

        assert_eq!(session.request_type, deserialized.request_type);
        assert_eq!(*session.request_status, *deserialized.request_status);

        let session_id = repo.create_session(session).await?;
        let retrieved_session = repo.get_session(session_id).await?;

        assert_eq!(retrieved_session.request_type, deserialized.request_type);
        assert_eq!(*retrieved_session.request_status, *deserialized.request_status);
        Ok(())
    }

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn test_performance_create_many_sessions(db: sqlx::PgPool) -> Result<(), DatabaseError> {
        let repo = make_repo_with_config(db).await;
        cleanup_and_setup(&repo).await;

        use std::time::Instant;
        let start = Instant::now();

        for _ in 0..ITERATIONS_V2 {
            let session = create_test_session();
            repo.create_session(session).await?;
        }

        let duration = start.elapsed();
        println!("Created 100 sessions in: {:?}", duration);

        Ok(())
    }
}
