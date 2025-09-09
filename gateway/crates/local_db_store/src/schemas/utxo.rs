use crate::errors::DatabaseError;
use async_trait::async_trait;
use persistent_storage::init::PostgresRepo;
use sqlx::FromRow;
use sqlx::types::chrono::{DateTime, Utc};

#[derive(Debug, Clone, FromRow)]
pub struct Utxo {
    pub id: i64,
    pub txid: String,
    pub vout: i32,
    pub amount: i64,
    pub rune_id: String,
    pub owner_address: String,
    pub nonce: Option<String>,
    pub status: String,
    pub block_height: Option<i64>,
    pub lock_expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[async_trait]
pub trait UtxoStorage {
    async fn insert_utxo(&self, utxo: Utxo) -> Result<Utxo, DatabaseError>;
    async fn update_status(&self, txid: &str, vout: i32, new_status: &str) -> Result<(), DatabaseError>;
    async fn list_unspent(&self, rune_id: &str) -> Result<Vec<Utxo>, DatabaseError>;
}

#[async_trait]
impl UtxoStorage for PostgresRepo {
    async fn insert_utxo(&self, utxo: Utxo) -> Result<Utxo, DatabaseError> {
        let rec = sqlx::query_as::<_, Utxo>(
            r#"
            INSERT INTO gateway.utxo
            (txid, vout, amount, rune_id, owner_address, nonce, status, block_height, lock_expires_at)
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)
            RETURNING *
            "#,
        )
            .bind(&utxo.txid)
            .bind(utxo.vout)
            .bind(utxo.amount)
            .bind(&utxo.rune_id)
            .bind(&utxo.owner_address)
            .bind(&utxo.nonce)
            .bind(&utxo.status)
            .bind(utxo.block_height)
            .bind(utxo.lock_expires_at)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| DatabaseError::BadRequest(e.to_string()))?;

        Ok(rec)
    }

    async fn update_status(&self, txid: &str, vout: i32, new_status: &str) -> Result<(), DatabaseError> {
        let rows = sqlx::query(
            r#"
            UPDATE gateway.utxo
            SET status = $1, updated_at = now()
            WHERE txid = $2 AND vout = $3
            "#,
        )
            .bind(new_status)
            .bind(txid)
            .bind(vout)
            .execute(&self.pool)
            .await
            .map_err(|e| DatabaseError::BadRequest(e.to_string()))?
            .rows_affected();

        if rows == 0 {
            return Err(DatabaseError::NotFound(format!("UTXO {txid}:{vout} not found")));
        }

        Ok(())
    }

    async fn list_unspent(&self, rune_id: &str) -> Result<Vec<Utxo>, DatabaseError> {
        let rows = sqlx::query_as::<_, Utxo>(
            r#"
            SELECT * FROM gateway.utxo
            WHERE rune_id = $1 AND status = 'unspent'
            ORDER BY amount ASC
            "#,
        )
            .bind(rune_id)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| DatabaseError::BadRequest(e.to_string()))?;

        Ok(rows)
    }
}
