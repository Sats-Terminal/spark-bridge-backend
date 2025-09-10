use crate::CoinSelector;
use async_trait::async_trait;
use gateway_local_db_store::schemas::utxo::*;
use persistent_storage::error::DbError;
use persistent_storage::init::PostgresRepo;

pub struct GreedySelector<'a> {
    pub repo: &'a PostgresRepo,
}

#[async_trait]
impl<'a> CoinSelector for GreedySelector<'a> {
    async fn select_utxos(&self, rune_id: &str, target_amount: i64) -> Result<Vec<Utxo>, DbError> {
        self.repo.select_and_lock_utxos(rune_id, target_amount).await
    }
}
