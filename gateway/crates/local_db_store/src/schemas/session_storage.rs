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
    pub request_type: RequestType, // enum
    pub request_status: Json<SessionStatus>, // json
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
pub trait SessionStorage {
    async fn create_session(&self, session_info: SessionInfo) -> Result<Uuid, DbError>;
    async fn update_session_status(&self, session_id: Uuid, status: SessionStatus) -> Result<(), DbError>;
}


#[async_trait]
impl SessionStorage for LocalDbStorage {
    async fn create_session(&self, session_info: SessionInfo) -> Result<Uuid, DbError> {
        let session_id = get_uuid();
        let query = r#"
            INSERT INTO gateway.session_requests (session_id, request_type, request_status)
            VALUES ($1, $2, $3)
        "#;
        sqlx::query(query)
            .bind(session_id)
            .bind(session_info.request_type)
            .bind(session_info.request_status)
            .execute(&self.get_conn().await?)
            .await
            .map_err(|e| DbError::BadRequest(e.to_string()))?;
        Ok(session_id)
    }

    async fn update_session_status(&self, session_id: Uuid, status: SessionStatus) -> Result<(), DbError> {
        let query = r#"
            UPDATE gateway.session_requests
            SET request_status = $1
            WHERE session_id = $2
        "#;
        sqlx::query(query)
            .bind(Json(status))
            .bind(session_id)
            .execute(&self.get_conn().await?)
            .await
            .map_err(|e| DbError::BadRequest(e.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use persistent_storage::error::DbError as DatabaseError;

    async fn make_repo(db: sqlx::PgPool) -> Arc<LocalDbStorage> {
        Arc::new(LocalDbStorage {
            postgres_repo: crate::storage::PostgresRepo { pool: db },
        })
    }

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
            request_status: Json::from(SessionStatus::Pending),
        }
    }

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn test_create_session(db: sqlx::PgPool) -> Result<(), DatabaseError> {
        let repo = make_repo(db).await;
        cleanup_and_setup(&repo).await;

        let session = create_test_session();
        let session_id = repo.create_session(session.clone()).await.unwrap();

        let row: (SessionStatus,) = sqlx::query_as("SELECT request_status FROM gateway.session_requests WHERE session_id = $1")
            .bind(session_id)
            .fetch_one(&repo.postgres_repo.pool)
            .await
            .unwrap();

        assert_eq!(row.0, session.request_status.0);
        Ok(())
    }

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn test_update_session_status(db: sqlx::PgPool) -> Result<(), DatabaseError> {
        let repo = make_repo(db).await;
        cleanup_and_setup(&repo).await;

        let session = create_test_session();
        let session_id = repo.create_session(session).await.unwrap();

        repo.update_session_status(session_id, SessionStatus::Completed).await.unwrap();

        let row: (SessionStatus,) = sqlx::query_as("SELECT request_status FROM gateway.session_requests WHERE session_id = $1")
            .bind(session_id)
            .fetch_one(&repo.postgres_repo.pool)
            .await
            .unwrap();

        assert_eq!(row.0, SessionStatus::Completed);
        Ok(())
    }
}