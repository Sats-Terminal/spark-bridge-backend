use async_trait::async_trait;
use crate::CoinSelector;
use gateway_local_db_store::schemas::utxo::*;
use gateway_local_db_store::errors::*;
use persistent_storage::init::PostgresRepo;

pub struct GreedySelector<'a> {
    pub repo: &'a PostgresRepo,
}

#[async_trait]
impl<'a> CoinSelector for GreedySelector<'a> {
    async fn select_utxos(
        &self,
        rune_id: &str,
        target_amount: i64,
    ) -> Result<Vec<Utxo>, DatabaseError> {
        self.repo.select_and_lock_utxos(rune_id, target_amount).await
    }
}
