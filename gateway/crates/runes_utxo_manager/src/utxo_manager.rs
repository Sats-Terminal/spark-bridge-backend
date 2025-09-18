use crate::traits::{CoinSelector, Utxo, UtxoStorage};
use async_trait::async_trait;
use chrono::NaiveDateTime;
use persistent_storage::error::DbError;
use sqlx::FromRow;
use std::sync::Arc;

pub struct GreedySelector {
    pub repo: Arc<dyn UtxoStorage>,
}

#[async_trait]
impl CoinSelector for GreedySelector {
    async fn select_utxos(&self, rune_id: &str, target_amount: i64) -> Result<Vec<Utxo>, DbError> {
        self.repo.select_and_lock_utxos(rune_id, target_amount).await
    }
}
