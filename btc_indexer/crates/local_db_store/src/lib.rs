use persistent_storage::error::DbError;
use persistent_storage::init::{PersistentRepoTrait, PostgresRepo};
use btc_indexer_config::DatabaseConfig;
use persistent_storage::config::PostgresDbCredentials;
use sqlx;
use sqlx::Type;
use bitcoin::{OutPoint, Address};
use ordinals::RuneId;
use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use std::str::FromStr;
use bitcoin::Network;
use url::Url;
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct LocalDbStorage {
    pub postgres_repo: PostgresRepo,
    pub network: Network,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash, Type)]
#[sqlx(rename_all = "snake_case", type_name = "WATCH_REQUEST_STATUS")]
pub enum WatchRequestStatus {
    Pending,
    Confirmed,
    Failed,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum WatchRequestErrorDetails {
    Timeout(String),
    InvalidData(String),
    InvalidRuneAmount(String),
    InvalidSatsAmount(String),
}

impl ToString for WatchRequestErrorDetails {
    fn to_string(&self) -> String {
        match self {
            WatchRequestErrorDetails::Timeout(msg) => msg.clone(),
            WatchRequestErrorDetails::InvalidData(msg) => msg.clone(),
            WatchRequestErrorDetails::InvalidRuneAmount(msg) => msg.clone(),
            WatchRequestErrorDetails::InvalidSatsAmount(msg) => msg.clone(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ValidationResult {
    pub watch_request_status: WatchRequestStatus,
    pub error_details: Option<WatchRequestErrorDetails>,
}

#[derive(Clone, Debug)]
pub struct WatchRequest {
    pub id: Uuid,
    pub outpoint: OutPoint,
    pub btc_address: Address,
    pub rune_id: Option<RuneId>,
    pub rune_amount: Option<u128>,
    pub sats_amount: Option<u64>,
    pub created_at: DateTime<Utc>,
    pub status: WatchRequestStatus,
    pub error_details: Option<WatchRequestErrorDetails>,
    pub callback_url: Url,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct WatchRequestRow {
    pub id: Uuid,
    pub outpoint: String,
    pub btc_address: String,
    pub rune_id: Option<String>,
    pub rune_amount: Option<i64>,
    pub sats_amount: Option<i64>,
    pub created_at: i64,
    pub status: WatchRequestStatus,
    pub error_details: Option<Json<WatchRequestErrorDetails>>,
    pub callback_url: String,
}

impl WatchRequest {
    fn into_row(self) -> WatchRequestRow {
        WatchRequestRow {
            id: self.id,
            outpoint: self.outpoint.to_string(),
            btc_address: self.btc_address.to_string(),
            rune_id: self.rune_id.map(|rune_id| rune_id.to_string()),
            rune_amount: self.rune_amount.map(|rune_amount| rune_amount as i64),
            sats_amount: self.sats_amount.map(|sats_amount| sats_amount as i64),
            created_at: self.created_at.timestamp_millis() as i64,
            status: self.status,
            error_details: self.error_details.map(|error_details| Json(error_details)),
            callback_url: self.callback_url.to_string(),
        }
    }

    fn from_row(row: WatchRequestRow, network: Network) -> Result<Self, DbError> {
        let outpoint = OutPoint::from_str(&row.outpoint)
            .map_err(|e| DbError::DecodeError(format!("Failed to parse outpoint: {}", e)))?;
        let btc_address = Address::from_str(&row.btc_address)
            .map_err(|e| DbError::DecodeError(format!("Failed to parse btc address: {}", e)))?
            .require_network(network)
            .map_err(|e| DbError::DecodeError(format!("Failed to require network: {}", e)))?;
        let rune_id = match row.rune_id {
            Some(rune_id) => Some(RuneId::from_str(&rune_id)
                .map_err(|e| DbError::DecodeError(format!("Failed to parse rune id: {}", e)))?),
            None => None,
        };
        Ok(Self {
            id: row.id,
            outpoint,
            btc_address,
            rune_id,
            rune_amount: row.rune_amount.map(|rune_amount| rune_amount as u128),
            sats_amount: row.sats_amount.map(|sats_amount| sats_amount as u64),
            created_at: DateTime::from_timestamp_millis(row.created_at).ok_or(DbError::DecodeError("Failed to parse created at".to_string()))?,
            status: row.status,
            error_details: row.error_details.map(|error_details| error_details.0),
            callback_url: Url::parse(&row.callback_url).map_err(|e| DbError::DecodeError(format!("Failed to parse callback url: {}", e)))?,
        })
    }
}

impl LocalDbStorage {
    pub async fn new(config: DatabaseConfig, network: Network) -> Result<Self, DbError> {
        let postgres_repo = PostgresRepo::from_config(PostgresDbCredentials { url: config.url }).await?;
        Ok(Self { postgres_repo, network })
    }

    pub async fn get_watch_request(&self, id: Uuid) -> Result<Option<WatchRequest>, DbError> {
        let response: Option<WatchRequestRow> = sqlx::query_as::<_, WatchRequestRow>(
            "SELECT id, outpoint, btc_address, rune_id, rune_amount, sats_amount, created_at, status, error_details, callback_url
            FROM btc_indexer.watch_request 
            WHERE id = $1",
        )
            .bind(id)
            .fetch_optional(&self.postgres_repo.pool)
            .await?;
        match response {
            Some(row) => Ok(Some(WatchRequest::from_row(row, self.network)?)),
            None => Ok(None),
        }
    }

    pub async fn get_all_unprocessed_watch_requests(&self) -> Result<Vec<WatchRequest>, DbError> {
        let rows = sqlx::query_as::<_, WatchRequestRow>(
            "SELECT id, outpoint, btc_address, rune_id, rune_amount, sats_amount, created_at, status, error_details, callback_url
            FROM btc_indexer.watch_request 
            WHERE status = 'pending'",
        )
            .fetch_all(&self.postgres_repo.pool)
            .await?;
        let watch_requests = rows.into_iter()
            .map(|row| WatchRequest::from_row(row, self.network))
            .collect::<Result<Vec<WatchRequest>, DbError>>()
            .map_err(|e| DbError::DecodeError(format!("Failed to decode watch request: {}", e)))?;
        Ok(watch_requests)
    }

    pub async fn insert_watch_request(&self, watch_request: WatchRequest) -> Result<(), DbError> {
        let row = watch_request.into_row();
        sqlx::query(
            "INSERT INTO btc_indexer.watch_request (id, outpoint, btc_address, rune_id, rune_amount, sats_amount, created_at, status, error_details, callback_url)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
        )
            .bind(row.id)
            .bind(row.outpoint)
            .bind(row.btc_address)
            .bind(row.rune_id)
            .bind(row.rune_amount)
            .bind(row.sats_amount)
            .bind(row.created_at)
            .bind(row.status)
            .bind(row.error_details)
            .bind(row.callback_url)
            .execute(&self.postgres_repo.pool)
            .await?;
        Ok(())
    }

    pub async fn update_watch_request_status(&self, id: Uuid, status: ValidationResult) -> Result<(), DbError> {
        sqlx::query(
            "UPDATE btc_indexer.watch_request 
            SET status = $1, error_details = $2
            WHERE id = $3",
        )
            .bind(status.watch_request_status)
            .bind(status.error_details.map(|error_details| Json(error_details)))
            .bind(id)
            .execute(&self.postgres_repo.pool)
            .await?;
        Ok(())
    }
}
