use serde::{Deserialize, Serialize};
use sqlx::{
    Acquire, FromRow, Postgres, Type,
    postgres::PgArguments,
    query::{Query, QueryAs},
    types::chrono::{DateTime, Utc},
};
use tracing::instrument;
use uuid::Uuid;

use crate::schemas::common::{ValuesMaxCapacity, ValuesToModifyInit};

const DB_NAME: &str = "runes_spark.user_request_stats";

#[derive(Debug, FromRow)]
pub struct UserRequestStats {
    uuid: Uuid,
    status: StatusTransferring,
    error: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Type, Copy, Clone)]
#[sqlx(rename_all = "snake_case", type_name = "STATUS_TRANSFERRING")]
pub enum StatusTransferring {
    Created,
    Processing,
    FinishedSuccess,
    FinishedError,
}

#[derive(Debug, Clone)]
pub struct Update {
    pub status: Option<StatusTransferring>,
    pub error: Option<String>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct Filter {
    pub uuid: Option<Uuid>,
    pub status: Option<StatusTransferring>,
    pub error: Option<String>,
}

impl ValuesMaxCapacity for Update {
    const MAX_CAPACITY: usize = 3;
}
impl ValuesMaxCapacity for Filter {
    const MAX_CAPACITY: usize = 3;
}

impl<'a> Filter {
    fn get_params_sets(&'a self) -> Vec<String> {
        const DEFAULT_INIT_PARAM: usize = 1;
        let (mut conditions, mut get_condition_closure) = Filter::init_values_to_modify(DEFAULT_INIT_PARAM);
        if self.uuid.is_some() {
            conditions.push(get_condition_closure("uuid"));
        }
        if self.status.is_some() {
            conditions.push(get_condition_closure("status"));
        }
        if self.error.is_some() {
            conditions.push(get_condition_closure("error"));
        }
        conditions
    }

    fn bind_params(&'a self, mut query: Query<'a, Postgres, PgArguments>) -> Query<Postgres, PgArguments> {
        if let Some(uuid) = self.uuid {
            query = query.bind(uuid);
        }
        if let Some(status) = self.status {
            query = query.bind(status);
        }
        if let Some(error) = &self.error {
            query = query.bind(error);
        }
        query
    }

    fn bind_params_user_req_stats(
        &'a self,
        mut query: QueryAs<'a, Postgres, UserRequestStats, PgArguments>,
    ) -> QueryAs<'a, Postgres, UserRequestStats, PgArguments> {
        if let Some(uuid) = self.uuid {
            query = query.bind(uuid);
        }
        if let Some(status) = self.status {
            query = query.bind(status);
        }
        if let Some(error) = &self.error {
            query = query.bind(error);
        }
        query
    }
}

impl<'a> Update {
    fn get_params_sets(&'a self) -> Vec<String> {
        const DEFAULT_INIT_PARAM: usize = 1;
        let (mut sets, mut get_condition_closure) = Filter::init_values_to_modify(DEFAULT_INIT_PARAM);
        if self.status.is_some() {
            sets.push(get_condition_closure("status"));
        }
        if self.error.is_some() {
            sets.push(get_condition_closure("error"));
        }
        if self.updated_at.is_some() {
            sets.push(get_condition_closure("updated_at"));
        }
        sets
    }

    fn bind_params(&'a self, mut query: Query<'a, Postgres, PgArguments>) -> Query<Postgres, PgArguments> {
        if let Some(status) = self.status {
            query = query.bind(status);
        }
        if let Some(error) = &self.error {
            query = query.bind(error);
        }
        if let Some(updated_at) = self.updated_at {
            query = query.bind(updated_at);
        }
        query
    }
}

impl UserRequestStats {
    #[instrument(skip(conn), level = "debug")]
    pub async fn insert(self, mut conn: sqlx::PgConnection) -> crate::error::Result<()> {
        let mut transaction = conn.begin().await?;
        sqlx::query(&format!(
            "INSERT INTO {DB_NAME} (uuid, status, error, created_at, updated_at) VALUES ($1, $2, $3, $4, $5)"
        ))
        .bind(self.uuid)
        .bind(self.status)
        .bind(self.error)
        .bind(self.created_at)
        .bind(self.updated_at)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(())
    }

    #[instrument(skip(conn), level = "debug")]
    pub async fn update(mut conn: sqlx::PgConnection, uuid: &Uuid, update: &Update) -> crate::error::Result<u64> {
        let sets = update.get_params_sets();
        if sets.is_empty() {
            return Ok(0);
        }
        let sql = format!(
            "UPDATE {DB_NAME} SET {} WHERE uuid = ${}",
            sets.join(", "),
            sets.len() + 1
        );
        let query = sqlx::query(&sql);
        let query = update.bind_params(query);
        let query = query.bind(uuid);

        let mut transaction = conn.begin().await?;
        let result = query.execute(&mut *transaction).await?;
        transaction.commit().await?;
        Ok(result.rows_affected())
    }

    #[instrument(skip(conn), level = "debug")]
    pub async fn remove(conn: &mut sqlx::PgConnection, filter: Option<&Filter>) -> crate::error::Result<u64> {
        match filter {
            None => Self::remove_all(conn).await,
            Some(f) => Self::remove_with_filter(conn, f).await,
        }
    }

    #[instrument(skip(conn), level = "debug")]
    async fn remove_all(conn: &mut sqlx::PgConnection) -> crate::error::Result<u64> {
        let mut transaction = conn.begin().await?;
        let result = sqlx::query(&format!("DELETE FROM {DB_NAME}"))
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        Ok(result.rows_affected())
    }

    #[instrument(skip(conn), level = "debug")]
    async fn remove_with_filter(conn: &mut sqlx::PgConnection, filter: &Filter) -> crate::error::Result<u64> {
        let conditions = filter.get_params_sets();
        if conditions.is_empty() {
            return Self::remove_all(conn).await;
        }

        let sql = format!("DELETE FROM {DB_NAME} WHERE {}", conditions.join(" AND "));
        let query = sqlx::query(&sql);
        let query = filter.bind_params(query);
        let mut transaction = conn.begin().await?;
        let result = query.execute(&mut *transaction).await?;
        transaction.commit().await?;
        Ok(result.rows_affected())
    }

    #[instrument(skip(conn), level = "debug")]
    pub async fn filter(
        mut conn: sqlx::PgConnection,
        filter: Option<&Filter>,
    ) -> crate::error::Result<Vec<UserRequestStats>> {
        match filter {
            None => Self::get_all(conn).await,
            Some(f) => Self::get_with_filter(conn, f).await,
        }
    }

    #[instrument(skip(conn), level = "debug")]
    async fn get_all(mut conn: sqlx::PgConnection) -> crate::error::Result<Vec<UserRequestStats>> {
        let mut transaction = conn.begin().await?;
        let results = sqlx::query_as::<_, UserRequestStats>(&format!(
            "SELECT uuid, status, error, created_at, updated_at FROM {DB_NAME}"
        ))
        .fetch_all(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(results)
    }

    #[instrument(skip(conn), level = "debug")]
    async fn get_with_filter(
        mut conn: sqlx::PgConnection,
        filter: &Filter,
    ) -> crate::error::Result<Vec<UserRequestStats>> {
        let conditions = filter.get_params_sets();
        if conditions.is_empty() {
            return Self::get_all(conn).await;
        }

        let sql = format!(
            "SELECT uuid, status, error, created_at, updated_at FROM {DB_NAME} WHERE {}",
            conditions.join(" AND ")
        );
        let query = sqlx::query_as::<_, UserRequestStats>(&sql);
        let query = filter.bind_params_user_req_stats(query);

        let mut transaction = conn.begin().await?;
        let results = query.fetch_all(&mut *transaction).await?;
        transaction.commit().await?;
        Ok(results)
    }
}
