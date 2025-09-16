use crate::greedy::{Utxo, UtxoStorage};
use async_trait::async_trait;
//use gateway_local_db_store::errors::*;
use gateway_local_db_store::storage::Storage;
use persistent_storage::error::DatabaseError;

#[async_trait]
pub trait CoinSelector {
    async fn select_utxos(
        &self,
        rune_id: &str,
        target_amount: i64,
    ) -> Result<Vec<Utxo>, DatabaseError>;
}

#[async_trait]
pub trait UtxoManager {
    async fn unlock_utxos(&self, utxo_ids: &[i64]) -> Result<(), DatabaseError>;
    async fn mark_spent(&self, utxo_ids: &[i64]) -> Result<(), DatabaseError>;
}

#[async_trait]
impl UtxoManager for Storage {
    async fn unlock_utxos(&self, utxo_ids: &[i64]) -> Result<(), DatabaseError> {
        UtxoStorage::unlock_utxos(self, utxo_ids).await
    }

    async fn mark_spent(&self, utxo_ids: &[i64]) -> Result<(), DatabaseError> {
        if utxo_ids.is_empty() {
            return Ok(());
        }

        let query = "UPDATE gateway.utxo SET status = 'spent', updated_at = now() WHERE id = ANY($1)";

        sqlx::query(query)
            .bind(utxo_ids)
            .execute(&self.postgres_repo.pool)
            .await
            .map_err(|e| DatabaseError::BadRequest(e.to_string()))?;

        Ok(())
    }
}
