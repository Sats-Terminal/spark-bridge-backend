use crate::storage::LocalDbStorage;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use persistent_storage::error::DbError;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;
use sqlx::types::Json;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredRuneMetadata {
    pub rune_id: String,
    pub rune_metadata: Option<Value>,
    pub wrune_metadata: Value,
    pub issuer_public_key: String,
    pub bitcoin_network: String,
    pub spark_network: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[async_trait]
pub trait RuneMetadataStorage: Send + Sync {
    async fn upsert_rune_metadata(
        &self,
        rune_id: String,
        rune_metadata: Option<Value>,
        wrune_metadata: Value,
        issuer_public_key: String,
        bitcoin_network: String,
        spark_network: String,
    ) -> Result<(), DbError>;

    async fn get_rune_metadata(&self, rune_id: &str) -> Result<Option<StoredRuneMetadata>, DbError>;

    async fn list_rune_metadata(&self) -> Result<Vec<StoredRuneMetadata>, DbError>;
}

#[derive(Debug, FromRow)]
struct RuneMetadataRow {
    rune_id: String,
    rune_metadata: Option<Json<Value>>,
    wrune_metadata: Json<Value>,
    issuer_public_key: String,
    bitcoin_network: String,
    spark_network: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[async_trait]
impl RuneMetadataStorage for LocalDbStorage {
    async fn upsert_rune_metadata(
        &self,
        rune_id: String,
        rune_metadata: Option<Value>,
        wrune_metadata: Value,
        issuer_public_key: String,
        bitcoin_network: String,
        spark_network: String,
    ) -> Result<(), DbError> {
        let rune_metadata_json = rune_metadata.map(Json);
        let wrune_metadata_json = Json(wrune_metadata);

        sqlx::query(
            r#"
            INSERT INTO gateway.rune_metadata_map (
                rune_id,
                rune_metadata,
                wrune_metadata,
                issuer_public_key,
                bitcoin_network,
                spark_network
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (rune_id) DO UPDATE SET
                rune_metadata = EXCLUDED.rune_metadata,
                wrune_metadata = EXCLUDED.wrune_metadata,
                issuer_public_key = EXCLUDED.issuer_public_key,
                bitcoin_network = EXCLUDED.bitcoin_network,
                spark_network = EXCLUDED.spark_network,
                updated_at = NOW()
            "#,
        )
        .bind(rune_id)
        .bind(rune_metadata_json)
        .bind(wrune_metadata_json)
        .bind(issuer_public_key)
        .bind(bitcoin_network)
        .bind(spark_network)
        .execute(&self.get_conn().await?)
        .await
        .map_err(|err| DbError::BadRequest(err.to_string()))?;

        Ok(())
    }

    async fn get_rune_metadata(&self, rune_id: &str) -> Result<Option<StoredRuneMetadata>, DbError> {
        let result: Option<RuneMetadataRow> = sqlx::query_as(
            r#"
            SELECT
                rune_id,
                rune_metadata,
                wrune_metadata,
                issuer_public_key,
                bitcoin_network,
                spark_network,
                created_at,
                updated_at
            FROM gateway.rune_metadata_map
            WHERE rune_id = $1
            "#,
        )
        .bind(rune_id)
        .fetch_optional(&self.get_conn().await?)
        .await
        .map_err(|err| DbError::BadRequest(err.to_string()))?;

        Ok(result.map(|row| StoredRuneMetadata {
            rune_id: row.rune_id,
            rune_metadata: row.rune_metadata.map(|json| json.0),
            wrune_metadata: row.wrune_metadata.0,
            issuer_public_key: row.issuer_public_key,
            bitcoin_network: row.bitcoin_network,
            spark_network: row.spark_network,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }))
    }

    async fn list_rune_metadata(&self) -> Result<Vec<StoredRuneMetadata>, DbError> {
        let rows: Vec<RuneMetadataRow> = sqlx::query_as(
            r#"
            SELECT
                rune_id,
                rune_metadata,
                wrune_metadata,
                issuer_public_key,
                bitcoin_network,
                spark_network,
                created_at,
                updated_at
            FROM gateway.rune_metadata_map
            ORDER BY created_at ASC
            "#,
        )
        .fetch_all(&self.get_conn().await?)
        .await
        .map_err(|err| DbError::BadRequest(err.to_string()))?;

        Ok(rows
            .into_iter()
            .map(|row| StoredRuneMetadata {
                rune_id: row.rune_id,
                rune_metadata: row.rune_metadata.map(|json| json.0),
                wrune_metadata: row.wrune_metadata.0,
                issuer_public_key: row.issuer_public_key,
                bitcoin_network: row.bitcoin_network,
                spark_network: row.spark_network,
                created_at: row.created_at,
                updated_at: row.updated_at,
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::Network;
    use persistent_storage::init::PostgresRepo;

    pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn test_upsert_and_get_metadata(pool: sqlx::PgPool) -> anyhow::Result<()> {
        let storage = LocalDbStorage {
            postgres_repo: PostgresRepo { pool },
            network: Network::Regtest,
        };

        let rune_metadata = serde_json::json!({
            "id": "123:1",
            "name": "TEST",
        });
        let wrune_metadata = serde_json::json!({
            "token_identifier": "test_identifier",
            "token_name": "TEST",
            "token_ticker": "TEST",
            "decimals": 0,
            "max_supply": 1000,
            "original_rune_id": "123:1"
        });

        storage
            .upsert_rune_metadata(
                "123:1".to_string(),
                Some(rune_metadata.clone()),
                wrune_metadata.clone(),
                "02abcdef".to_string(),
                "regtest".to_string(),
                "Regtest".to_string(),
            )
            .await?;

        let fetched = storage.get_rune_metadata("123:1").await?.expect("metadata not found");
        assert_eq!(fetched.rune_id, "123:1");
        assert_eq!(fetched.rune_metadata, Some(rune_metadata));
        assert_eq!(fetched.wrune_metadata, wrune_metadata);
        assert_eq!(fetched.issuer_public_key, "02abcdef");
        assert_eq!(fetched.bitcoin_network, "regtest");
        assert_eq!(fetched.spark_network, "Regtest");

        let all = storage.list_rune_metadata().await?;
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].rune_id, "123:1");
        Ok(())
    }
}
