use btc_indexer_api::api::BtcTxReview;
use global_utils::common_types::TxIdWrapped;
use persistent_storage::error::DbError;
use persistent_storage::init::PersistentDbConn;
use serde::{Deserialize, Serialize};
use sqlx::{Connection, FromRow, Row, types::Json};
use tracing::instrument;

#[derive(Debug, FromRow, Clone, Serialize, Deserialize)]
pub struct TxIdIndexed {
    pub tx_id: TxIdWrapped,
    pub block_height: Option<u64>,
    pub btc_tx_review: Option<BtcTxReview>,
    pub transaction: Option<titan_client::Transaction>,
}

impl TxIdIndexed {
    #[instrument(skip(conn), level = "trace")]
    pub async fn insert(&self, conn: &mut PersistentDbConn) -> Result<(), DbError> {
        let mut transaction = conn.begin().await?;

        let _ = sqlx::query(
            "INSERT INTO runes_spark.tx_ids_indexed (tx_id, block_height, btc_tx_review, transaction)
            VALUES ($1, $2, $3, $4) ON CONFLICT (tx_id)
            DO UPDATE SET block_height = $2, btc_tx_review = $3, transaction = $4",
        )
        .bind(&self.tx_id)
        .bind(self.block_height.map(|x| x as i64))
        .bind(Json(&self.btc_tx_review))
        .bind(Json(&self.transaction))
        .execute(&mut *transaction)
        .await
        .map_err(|e| DbError::BadRequest(e.to_string()))?;
        transaction.commit().await?;
        Ok(())
    }

    #[instrument(skip(conn), level = "trace")]
    pub async fn get(conn: &mut PersistentDbConn, tx_id: &TxIdWrapped) -> Result<Option<TxIdIndexed>, DbError> {
        let mut transaction = conn.begin().await?;
        let result: Option<(
            Option<i64>,
            Option<Json<BtcTxReview>>,
            Option<Json<titan_client::Transaction>>,
        )> = sqlx::query_as(
            "SELECT block_height, btc_tx_review, transaction 
            FROM runes_spark.tx_ids_indexed 
            WHERE tx_id = $1",
        )
        .bind(&tx_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|e| DbError::BadRequest(e.to_string()))?;
        transaction.commit().await?;

        Ok(result.map(|(block_height, btc_tx_review, transaction)| TxIdIndexed {
            tx_id: tx_id.clone(),
            block_height: block_height.map(|x| x as u64),
            btc_tx_review: btc_tx_review.map(|x| x.0),
            transaction: transaction.map(|x| x.0),
        }))
    }
}
