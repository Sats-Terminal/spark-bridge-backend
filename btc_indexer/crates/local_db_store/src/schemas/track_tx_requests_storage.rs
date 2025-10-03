use crate::init::LocalDbStorage;
use crate::schemas::tx_tracking_storage::{Id, TrackedRawTxStatus};
use bitcoin::OutPoint;
use btc_indexer_api::api::{BtcTxReview, TrackTxRequest};
use global_utils::common_types::{TxIdWrapped, UrlWrapped};
use persistent_storage::error::DbError;
use persistent_storage::init::StorageHealthcheck;
use serde::{Deserialize, Serialize};
use sqlx::Acquire;
use sqlx::types::Json;
use sqlx::types::chrono::{DateTime, Utc};
use titan_client::Transaction;
use tracing::instrument;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, sqlx::Type, Clone, Copy, Eq, PartialEq, Hash)]
#[sqlx(rename_all = "snake_case", type_name = "BTC_TRACK_TX_REQUEST_STATUS")]
pub enum TrackedReqStatus {
    Pending,
    Finished,
    FailedToSend,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct TxTrackingRequestsStorage {
    pub uuid: Uuid,
    pub tracked_tx_id: Id,
    pub callback_url: UrlWrapped,
    pub created_at: DateTime<Utc>,
    pub status: TrackedReqStatus,
}

#[derive(Debug, Clone)]
pub struct TxTrackingRequestsToSendResponse {
    pub uuid: Uuid,
    pub out_point: OutPoint,
    pub callback_url: UrlWrapped,
    pub review: BtcTxReview,
    pub transaction: Transaction,
}

#[async_trait::async_trait]
pub trait TxRequestsTrackingStorageTrait: Send + Sync + StorageHealthcheck {
    /// Inserts appropriate entry to `btc_indexer.tx_tracking` and `btc_indexer.tx_tracking_requests`
    async fn track_tx_request(&self, uuid: Uuid, req: &TrackTxRequest) -> Result<(), DbError>;
    async fn get_values_to_send_response(&self) -> Result<Vec<TxTrackingRequestsToSendResponse>, DbError>;
    /// Finalized request, sets status to finalized when request was sent to the recipient
    async fn finalize_tx_request(&self, uuid: Uuid, status: TrackedReqStatus) -> Result<(), DbError>;
}

#[async_trait::async_trait]
impl TxRequestsTrackingStorageTrait for LocalDbStorage {
    #[instrument(level = "trace", skip(self), ret)]
    async fn track_tx_request(&self, uuid: Uuid, req: &TrackTxRequest) -> Result<(), DbError> {
        let mut conn = self.postgres_repo.get_conn().await?;
        let mut transaction = conn.begin().await?;

        let id: (i32,) = sqlx::query_as(
            "INSERT INTO btc_indexer.tx_tracking (tx_id, v_out, status, created_at, amount, rune_id)
                 VALUES ($1, $2, $3, $4, $5, $6)
                 ON CONFLICT (tx_id, v_out) DO UPDATE SET tx_id = EXCLUDED.tx_id
                 RETURNING id;",
        )
        .bind(TxIdWrapped(req.out_point.txid))
        .bind(req.out_point.vout as i32)
        .bind(TrackedRawTxStatus::Pending)
        .bind(Utc::now())
        .bind(req.rune_amount as i64)
        .bind(req.rune_id.to_string())
        .fetch_one(&mut *transaction)
        .await
        .map_err(|e| DbError::BadRequest(e.to_string()))?;

        let _ = sqlx::query(
            "INSERT INTO btc_indexer.tx_tracking_requests (uuid, tracked_tx_id, callback_url, created_at, status)
                  VALUES ($1, $2, $3, $4, $5);",
        )
        .bind(uuid)
        .bind(id.0)
        .bind(&req.callback_url)
        .bind(Utc::now())
        .bind(TrackedReqStatus::Pending)
        .execute(&mut *transaction)
        .await
        .map_err(|e| DbError::BadRequest(e.to_string()))?;

        transaction.commit().await?;
        Ok(())
    }

    #[instrument(level = "trace", skip(self), ret)]
    async fn get_values_to_send_response(&self) -> Result<Vec<TxTrackingRequestsToSendResponse>, DbError> {
        let mut conn = self.postgres_repo.get_conn().await?;
        let mut transaction = conn.begin().await?;

        let req_to_answer: Vec<(TxIdWrapped, i32, Json<BtcTxReview>,UrlWrapped, Uuid, Json<Transaction>)> = sqlx::query_as(
            " SELECT inner_table.tx_id, inner_table.v_out, inner_table.btc_tx_review, req_table.callback_url, req_table.uuid, inner_table.transaction
                    FROM btc_indexer.tx_tracking_requests req_table
                    JOIN btc_indexer.tx_tracking inner_table ON req_table.tracked_tx_id = inner_table.id
                    WHERE inner_table.status = $1 AND req_table.status = $2;",
        )
        .bind(TrackedRawTxStatus::Finalized)
        .bind(TrackedReqStatus::Pending)
        .fetch_all(&mut *transaction)
        .await
        .map_err(|e| DbError::BadRequest(e.to_string()))?;
        let req_to_answer = req_to_answer
            .into_iter()
            .map(
                |(tx_id, v_out, review, url, uuid, transaction)| TxTrackingRequestsToSendResponse {
                    uuid,
                    out_point: OutPoint {
                        txid: tx_id.0,
                        vout: v_out as u32,
                    },
                    callback_url: url,
                    review: review.0,
                    transaction: transaction.0,
                },
            )
            .collect();

        transaction.commit().await?;
        Ok(req_to_answer)
    }

    #[instrument(level = "trace", skip(self), ret)]
    async fn finalize_tx_request(&self, uuid: Uuid, status: TrackedReqStatus) -> Result<(), DbError> {
        let mut conn = self.postgres_repo.get_conn().await?;
        let mut transaction = conn.begin().await?;

        let _ = sqlx::query("UPDATE btc_indexer.tx_tracking_requests SET status = $1 WHERE uuid = $2;")
            .bind(status)
            .bind(uuid)
            .execute(&mut *transaction)
            .await
            .map_err(|e| DbError::BadRequest(e.to_string()))?;

        transaction.commit().await?;
        Ok(())
    }
}
