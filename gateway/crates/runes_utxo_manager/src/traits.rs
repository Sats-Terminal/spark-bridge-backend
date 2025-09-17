use async_trait::async_trait;
use bitcoin::hashes::serde::{Deserialize, Serialize};
use chrono::NaiveDateTime;
use persistent_storage::error::DbError;
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow)]
pub struct Utxo {
    pub id: i32,
    pub txid: String,
    pub vout: i32,
    pub amount: i64,
    pub rune_id: String,
    pub sats_amount: Option<i64>,
    pub owner_pubkey: String,
    pub status: UtxoStatus,
    pub block_height: Option<i64>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Serialize, Deserialize, sqlx::Type, Clone, Copy, Eq, PartialEq, Hash)]
#[sqlx(rename_all = "snake_case", type_name = "UTXO_STATUS")]
pub enum UtxoStatus {
    Pending,
    Spent,
    Unspent,
    Locked,
    Archived,
}

#[async_trait]
pub trait CoinSelector: Send + Sync {
    async fn select_utxos(&self, rune_id: &str, target_amount: i64) -> Result<Vec<Utxo>, DbError>;
}

#[async_trait]
pub trait UtxoManager: Send + Sync {
    async fn unlock_utxos(&self, utxo_ids: &[i64]) -> Result<(), DbError>;
    async fn mark_spent(&self, utxo_ids: &[i64]) -> Result<(), DbError>;
}

#[async_trait]
pub trait UtxoStorage: Send + Sync {
    async fn insert_utxo(&self, utxo: Utxo) -> Result<Utxo, DbError>;
    async fn update_status(&self, txid: &str, vout: i32, new_status: UtxoStatus) -> Result<(), DbError>;
    async fn insert_pending_utxo(&self, utxo: Vec<Utxo>) -> Result<Utxo, DbError>;
    async fn list_unspent(&self, rune_id: &str) -> Result<Vec<Utxo>, DbError>;
    async fn confirm_pending_utxo(&self, txid: bitcoin::Txid, block_height: i64) -> Result<(), DbError>;
    async fn select_and_lock_utxos(&self, rune_id: &str, target_amount: i64) -> Result<Vec<Utxo>, DbError>;
    async fn unlock_utxos_ids(&self, utxo_ids: &[i64]) -> Result<(), DbError>;
    async fn mark_spent_ids(&self, utxo_ids: &[i64]) -> Result<(), DbError>;
    async fn set_block_height(&self, txid: bitcoin::Txid, block_height: i64) -> Result<(), DbError>;
}
