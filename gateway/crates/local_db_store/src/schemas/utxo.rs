use crate::errors::DatabaseError;
use async_trait::async_trait;
use chrono::NaiveDateTime;
use persistent_storage::init::PostgresRepo;
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow)]
pub struct Utxo {
    pub id: i32,
    pub txid: String,
    pub vout: i32,
    pub amount: i64,
    pub rune_id: String,
    pub owner_pubkey: String,  // изменил с owner_address
    pub status: String,
    pub block_height: Option<i64>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[async_trait]
pub trait UtxoStorage {
    async fn insert_utxo(&self, utxo: Utxo) -> Result<Utxo, DatabaseError>;
    async fn update_status(&self, txid: &str, vout: i32, new_status: &str) -> Result<(), DatabaseError>;
    async fn list_unspent(&self, rune_id: &str) -> Result<Vec<Utxo>, DatabaseError>;
    async fn select_and_lock_utxos(&self, rune_id: &str, target_amount: i64) -> Result<Vec<Utxo>, DatabaseError>;
    async fn unlock_utxos(&self, utxo_ids: &[i64]) -> Result<(), DatabaseError>;
}

#[async_trait]
impl UtxoStorage for PostgresRepo {
    async fn insert_utxo(&self, utxo: Utxo) -> Result<Utxo, DatabaseError> {
        let rec = sqlx::query_as::<_, Utxo>(
            r#"
            INSERT INTO gateway.utxo
            (txid, vout, amount, rune_id, owner_pubkey, status, block_height)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING *
            "#,
        )
            .bind(&utxo.txid)
            .bind(utxo.vout)
            .bind(utxo.amount)
            .bind(&utxo.rune_id)
            .bind(&utxo.owner_pubkey)
            .bind(&utxo.status)
            .bind(utxo.block_height)
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

    async fn select_and_lock_utxos(&self, rune_id: &str, target_amount: i64) -> Result<Vec<Utxo>, DatabaseError> {
        let mut tx = self.pool.begin().await
            .map_err(|e| DatabaseError::BadRequest(format!("Failed to begin transaction: {}", e)))?;

        let candidates = sqlx::query_as::<_, Utxo>(
            r#"
        SELECT *
        FROM gateway.utxo
        WHERE rune_id = $1 AND status = 'unspent'
        ORDER BY amount ASC
        FOR UPDATE SKIP LOCKED
        "#,
        )
            .bind(rune_id)
            .fetch_all(&mut *tx)
            .await
            .map_err(|e| DatabaseError::BadRequest(e.to_string()))?;

        let mut selected = Vec::new();
        let mut total = 0;
        for u in candidates {
            if total < target_amount {
                total += u.amount;
                selected.push(u);
            } else {
                break;
            }
        }

        if total < target_amount {
            tx.rollback().await.ok();
            return Err(DatabaseError::BadRequest("Not enough funds".into()));
        }

        let ids: Vec<i32> = selected.iter().map(|u| u.id).collect();
        let locked_utxos = sqlx::query_as::<_, Utxo>(
            r#"
        UPDATE gateway.utxo
        SET status = 'locked', updated_at = now()
        WHERE id = ANY($1)
        RETURNING *
        "#,
        )
            .bind(&ids)
            .fetch_all(&mut *tx)
            .await
            .map_err(|e| DatabaseError::BadRequest(e.to_string()))?;

        tx.commit().await
            .map_err(|e| DatabaseError::BadRequest(format!("Failed to commit transaction: {}", e)))?;

        Ok(locked_utxos)
    }

    async fn unlock_utxos(&self, utxo_ids: &[i64]) -> Result<(), DatabaseError> {
        if utxo_ids.is_empty() {
            return Ok(());
        }

        let query = format!(
            "UPDATE gateway.utxo SET status = 'unspent', updated_at = now() WHERE id = ANY($1)"
        );

        sqlx::query(&query)
            .bind(utxo_ids)
            .execute(&self.pool)
            .await
            .map_err(|e| DatabaseError::BadRequest(e.to_string()))?;

        Ok(())
    }
}