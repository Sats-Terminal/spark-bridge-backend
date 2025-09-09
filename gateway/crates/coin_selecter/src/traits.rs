use async_trait::async_trait;
use gateway_local_db_store::schemas::utxo::*;
use gateway_local_db_store::errors::*;
#[async_trait]
pub trait CoinSelector {
    async fn select_utxos(
        &self,
        rune_id: &str,
        target_amount: i64,
    ) -> Result<Vec<Utxo>, DatabaseError>;
}
