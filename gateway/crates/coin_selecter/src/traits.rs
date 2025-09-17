use crate::greedy::{Utxo, UtxoStorage};
use async_trait::async_trait;
//use gateway_local_db_store::errors::*;
use gateway_local_db_store::storage::Storage;
use persistent_storage::error::DatabaseError;

#[async_trait]
pub trait CoinSelector {
    async fn select_utxos(&self, rune_id: &str, target_amount: i64) -> Result<Vec<Utxo>, DatabaseError>;
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
        UtxoStorage::mark_spent(self,utxo_ids).await
    }
}
