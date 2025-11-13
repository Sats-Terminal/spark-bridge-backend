use crate::storage::LocalDbStorage;
use async_trait::async_trait;
use persistent_storage::error::DbError;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use bitcoin::Txid;
use bitcoin::hashes::Hash;
use hex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxInfo {
    pub txid: Txid,
    pub block_height: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TxInfoRow {
    pub txid: [u8; 32],
    pub block_height: i64,
}

impl From<TxInfo> for TxInfoRow {
    fn from(val: TxInfo) -> Self {
        TxInfoRow {
            txid: val.txid.to_byte_array(),
            block_height: val.block_height as i64,
        }
    }
}

#[async_trait]
pub trait TxsStorage: Send + Sync {
    async fn insert_txs(&self, txs: Vec<TxInfo>) -> Result<(), DbError>;
    async fn exists(&self, txid: Txid) -> Result<bool, DbError>;
}

#[async_trait]
impl TxsStorage for LocalDbStorage {
    async fn insert_txs(&self, txs: Vec<TxInfo>) -> Result<(), DbError> {
        let rows = txs.into_iter().map(|tx| tx.into()).collect::<Vec<TxInfoRow>>();

        let values = rows
            .iter()
            .map(|tx| format!("(decode('{}', 'hex'), {})", hex::encode(tx.txid), tx.block_height))
            .collect::<Vec<String>>()
            .join(",\n");

        let query_str = format!(
            "INSERT INTO btc_indexer.txs (txid, block_height) VALUES {}",
            values
        );

        sqlx::query(&query_str)
            .execute(&self.postgres_repo.pool)
            .await?;

        Ok(())
    }

    async fn exists(&self, txid: Txid) -> Result<bool, DbError> {
        let row = sqlx::query(
            "SELECT 1 FROM btc_indexer.txs WHERE txid = $1",
        )
            .bind(txid.to_byte_array())
            .fetch_optional(&self.postgres_repo.pool)
            .await?;
        Ok(row.is_some())
    }
}
