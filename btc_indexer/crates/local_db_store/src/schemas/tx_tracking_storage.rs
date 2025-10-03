use crate::init::LocalDbStorage;
use bitcoin::OutPoint;
use btc_indexer_api::api::{Amount, BtcTxReview, VOut};
use global_utils::common_types::TxIdWrapped;
use ordinals::RuneId;
use persistent_storage::error::DbError;
use persistent_storage::init::StorageHealthcheck;
use serde::{Deserialize, Serialize};
use sqlx::Acquire;
use sqlx::types::Json;
use sqlx::types::chrono::{DateTime, Utc};
use std::str::FromStr;
use titan_client::Transaction;
use tracing::instrument;

#[derive(Debug, Serialize, Deserialize, sqlx::Type, Clone, Copy, Eq, PartialEq, Hash)]
#[sqlx(rename_all = "snake_case", type_name = "BTC_TRACKED_TX_STATUS")]
pub enum TrackedRawTxStatus {
    Pending,
    Finalized,
}

pub type Id = u64;

#[derive(Debug, Clone)]
pub struct TxTrackingStorage {
    pub id: Id,
    pub tx_id: TxIdWrapped,
    pub v_out: VOut,
    pub status: TrackedRawTxStatus,
    pub btc_tx_review: BtcTxReview,
    pub transaction: Transaction,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct TxToUpdateStatus {
    pub tx_id: TxIdWrapped,
    pub v_out: VOut,
    pub amount: Amount,
    pub rune_id: RuneId,
}

#[async_trait::async_trait]
pub trait TxTrackingStorageTrait: Send + Sync + StorageHealthcheck {
    async fn get_txs_to_update_status(&self) -> Result<Vec<TxToUpdateStatus>, DbError>;
    async fn insert_tx_tracking_report(
        &self,
        outpoint: OutPoint,
        btc_tx_review: &BtcTxReview,
        transaction: &Transaction,
    ) -> Result<(), DbError>;
}

#[async_trait::async_trait]
impl TxTrackingStorageTrait for LocalDbStorage {
    #[instrument(level = "trace", skip(self), ret)]
    async fn get_txs_to_update_status(&self) -> Result<Vec<TxToUpdateStatus>, DbError> {
        let mut conn = self.postgres_repo.get_conn().await?;
        let mut transaction = conn.begin().await?;

        let stored_values: Vec<(TxIdWrapped, i32, i64, String)> =
            sqlx::query_as("SELECT tx_id, v_out, amount, rune_id FROM btc_indexer.tx_tracking WHERE status = $1;")
                .bind(TrackedRawTxStatus::Pending)
                .fetch_all(&mut *transaction)
                .await
                .map_err(|e| DbError::BadRequest(e.to_string()))?;
        let stored_values: Vec<Result<TxToUpdateStatus, DbError>> = stored_values
            .into_iter()
            .map(|(tx_id, v_out, amount, rune_id)| {
                Ok(TxToUpdateStatus {
                    tx_id,
                    v_out: v_out as u32,
                    amount: amount as u64,
                    rune_id: RuneId::from_str(&rune_id).map_err(|e| {
                        DbError::DecodeError(format!("Failed to decode RuneId from string: {rune_id}, err: {e}"))
                    })?,
                })
            })
            .collect();
        let stored_values = stored_values
            .into_iter()
            .collect::<Result<Vec<TxToUpdateStatus>, DbError>>()?;
        transaction.commit().await?;
        Ok(stored_values)
    }

    #[instrument(level = "trace", skip(self), ret)]
    async fn insert_tx_tracking_report(
        &self,
        outpoint: OutPoint,
        btc_tx_review: &BtcTxReview,
        titan_tx: &Transaction,
    ) -> Result<(), DbError> {
        let mut conn = self.postgres_repo.get_conn().await?;
        let mut transaction = conn.begin().await?;
        let _ = sqlx::query(
            "UPDATE btc_indexer.tx_tracking
                 SET btc_tx_review = $1, transaction = $2, status = $3
                 WHERE tx_id = $4 AND v_out = $5;",
        )
        .bind(Json(btc_tx_review))
        .bind(Json(titan_tx))
        .bind(TrackedRawTxStatus::Finalized)
        .bind(TxIdWrapped(outpoint.txid))
        .bind(outpoint.vout as i32)
        .execute(&mut *transaction)
        .await
        .map_err(|e| DbError::BadRequest(e.to_string()))?;
        transaction.commit().await?;
        Ok(())
    }
}
