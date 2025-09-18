use crate::storage::LocalDbStorage;
use async_trait::async_trait;
use persistent_storage::error::DbError;
use sqlx::{Postgres, Transaction};
use serde::{Deserialize, Serialize};
use bitcoin::{Transaction as BitcoinTransaction};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Utxo {
    pub txid: String,
    pub vout: i32,
    pub btc_address: String,
    pub transaction: Option<BitcoinTransaction>,
    pub amount: i64,
    pub rune_id: String,
    pub status: UtxoStatus,
}

#[derive(Debug, Serialize, Deserialize, sqlx::Type, Clone, Copy, Eq, PartialEq, Hash)]
#[sqlx(rename_all = "snake_case", type_name = "UTXO_STATUS")]
pub enum UtxoStatus {
    Pending,
    Confirmed,
    Spent,
}

#[async_trait]
pub trait UtxoStorage: Send + Sync {
    async fn insert_utxo(&self, utxo: Utxo) -> Result<Utxo, DbError>;
    async fn update_status(&self, txid: &str, vout: i32, new_status: UtxoStatus, transaction: Option<BitcoinTransaction>) -> Result<(), DbError>;
    async fn list_unspent(&self, rune_id: &str) -> Result<Vec<Utxo>, DbError>;
    async fn select_and_lock_utxos(&self, rune_id: &str, target_amount: i64) -> Result<Vec<Utxo>, DbError>;
}

#[async_trait]
impl UtxoStorage for LocalDbStorage {
    async fn insert_utxo(&self, utxo: Utxo) -> Result<Utxo, DbError> {
        let rec = sqlx::query_as::<_, Utxo>(
            r#"
    INSERT INTO gateway.utxo
        (txid, vout, amount, sats_amount, rune_id, owner_pubkey, status, block_height)
    VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
    ON CONFLICT (txid, vout) DO UPDATE
    SET amount = EXCLUDED.amount,
        sats_amount = EXCLUDED.sats_amount,
        rune_id = EXCLUDED.rune_id,
        owner_pubkey = EXCLUDED.owner_pubkey,
        status = EXCLUDED.status,
        block_height = EXCLUDED.block_height,
        updated_at = NOW()
    RETURNING *
    "#,
        )
        .bind(&utxo.txid)
        .bind(utxo.vout)
        .bind(utxo.amount)
        .bind(utxo.sats_amount)
        .bind(&utxo.rune_id)
        .bind(&utxo.owner_pubkey)
        .bind(&utxo.status)
        .bind(utxo.block_height)
        .fetch_one(&self.postgres_repo.pool)
        .await
        .map_err(|e| DbError::BadRequest(e.to_string()))?;

        Ok(rec)
    }

    async fn update_status(&self, txid: &str, vout: i32, new_status: UtxoStatus, transaction: Option<BitcoinTransaction>) -> Result<(), DbError> {
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
        .execute(&self.get_conn().await?)
        .await
        .map_err(|e| DbError::BadRequest(e.to_string()))?
        .rows_affected();

        if rows == 0 {
            return Err(DbError::NotFound(format!("UTXO {txid}:{vout} not found")));
        }

        Ok(())
    }

    async fn list_unspent(&self, rune_id: &str) -> Result<Vec<Utxo>, DbError> {
        let rows = sqlx::query_as::<_, Utxo>(
            r#"
        SELECT * FROM gateway.utxo
        WHERE rune_id = $1 AND status IN ('unspent', 'pending')
        ORDER BY
            CASE WHEN status = 'unspent' THEN 0 ELSE 1 END,
            amount ASC
        "#,
        )
        .bind(rune_id)
        .fetch_all(&self.get_conn().await?)
        .await
        .map_err(|e| DbError::BadRequest(e.to_string()))?;

        Ok(rows)
    }

    async fn select_and_lock_utxos(&self, rune_id: &str, target_amount: i64) -> Result<Vec<Utxo>, DbError> {
        let mut tx = self
            .postgres_repo
            .pool
            .begin()
            .await
            .map_err(|e| DbError::BadRequest(format!("Failed to begin transaction: {}", e)))?;

        let candidates = get_candidate_utxos_for_update(rune_id, &mut tx).await?;

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
            return Err(DbError::BadRequest("Not enough funds".into()));
        }

        let ids: Vec<i32> = selected.iter().map(|u| u.id).collect();
        let locked_utxos = lock_utxos_by_ids(&ids, &mut tx).await?;

        tx.commit()
            .await
            .map_err(|e| DbError::BadRequest(format!("Failed to commit transaction: {}", e)))?;

        Ok(locked_utxos)
    }
}

async fn get_candidate_utxos_for_update(
    rune_id: &str,
    tx: &mut Transaction<'_, Postgres>,
) -> Result<Vec<Utxo>, DbError> {
    let candidates = sqlx::query_as::<_, Utxo>(
        r#"
        SELECT *
        FROM gateway.utxo
        WHERE rune_id = $1 AND status IN ('unspent', 'pending')
        ORDER BY amount ASC
        FOR UPDATE SKIP LOCKED
        "#,
    )
    .bind(rune_id)
    .fetch_all(&mut **tx)
    .await
    .map_err(|e| DbError::BadRequest(e.to_string()))?;

    Ok(candidates)
}

async fn lock_utxos_by_ids(utxo_ids: &[i32], tx: &mut Transaction<'_, Postgres>) -> Result<Vec<Utxo>, DbError> {
    let locked_utxos = sqlx::query_as::<_, Utxo>(
        r#"
        UPDATE gateway.utxo
        SET status = 'locked', updated_at = now()
        WHERE id = ANY($1)
        RETURNING *
        "#,
    )
    .bind(utxo_ids)
    .fetch_all(&mut **tx)
    .await
    .map_err(|e| DbError::BadRequest(e.to_string()))?;

    Ok(locked_utxos)
}
