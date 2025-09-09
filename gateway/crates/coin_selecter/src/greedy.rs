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
        let mut utxos = self.repo.list_unspent(rune_id).await?;

        utxos.sort_by_key(|u| u.amount);

        let mut selected = Vec::new();
        let mut total = 0;

        for u in utxos {
            selected.push(u.clone());
            total += u.amount;
            if total >= target_amount {
                break;
            }
        }

        if total < target_amount {
            return Err(DatabaseError::BadRequest("Not enough funds".into()));
        }

        for u in &selected {
            self.repo
                .update_status(&u.txid, u.vout, "locked")
                .await?;
        }

        Ok(selected)
    }
}
