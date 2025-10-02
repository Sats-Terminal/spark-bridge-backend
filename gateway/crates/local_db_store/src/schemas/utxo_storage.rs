use crate::storage::LocalDbStorage;
use async_trait::async_trait;
use bitcoin::OutPoint;
use global_utils::conversion::decode_address;
use persistent_storage::error::DbError;
use serde::{Deserialize, Serialize};
use sqlx::{Postgres, Transaction};
use tracing::instrument;

#[derive(Debug, Clone)]
pub struct Utxo {
    pub out_point: OutPoint,
    pub btc_address: bitcoin::Address,
    pub rune_amount: u64,
    pub rune_id: String,
    pub status: UtxoStatus,
    pub sats_fee_amount: u64,
}

impl Utxo {
    fn from_row(row: UtxoRow, network: bitcoin::Network) -> Result<Self, DbError> {
        Ok(Self {
            out_point: row
                .out_point
                .parse()
                .map_err(|e| DbError::DecodeError(format!("Failed to parse out point: {}", e)))?,
            btc_address: decode_address(&row.btc_address, network)
                .map_err(|e| DbError::DecodeError(format!("Failed to decode btc address: {}", e)))?,
            rune_amount: row.rune_amount as u64,
            rune_id: row.rune_id,
            status: row.status,
            sats_fee_amount: row.sats_fee_amount as u64,
        })
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct UtxoRow {
    pub out_point: String,
    pub btc_address: String,
    pub rune_amount: i64,
    pub rune_id: String,
    pub status: UtxoStatus,
    pub sats_fee_amount: i64,
}

#[derive(Debug, Serialize, Deserialize, sqlx::Type, Clone, Copy, Eq, PartialEq, Hash)]
#[sqlx(rename_all = "snake_case", type_name = "UTXO_STATUS")]
pub enum UtxoStatus {
    Pending,
    Confirmed,
    Spent,
}

#[derive(Debug, Serialize, Deserialize, sqlx::Type, Clone, Copy, Eq, PartialEq, Hash)]
#[sqlx(rename_all = "snake_case", type_name = "REQUEST_STATUS")]
pub enum RequestStatus {
    Pending,
    Processing,
    Completed,
    Failed,
    Cancelled,
}

#[async_trait]
pub trait UtxoStorage: Send + Sync {
    async fn insert_utxo(&self, utxo: Utxo) -> Result<Utxo, DbError>;
    async fn update_status(&self, out_point: OutPoint, new_status: UtxoStatus) -> Result<(), DbError>;
    async fn list_unspent(&self, rune_id: String) -> Result<Vec<Utxo>, DbError>;
    async fn select_utxos_for_amount(&self, rune_id: String, target_amount: u64) -> Result<Vec<Utxo>, DbError>;
    async fn get_utxo(&self, out_point: OutPoint) -> Result<Option<Utxo>, DbError>;
    async fn delete_utxo(&self, out_point: OutPoint) -> Result<(), DbError>;
    async fn update_sats_fee_amount(&self, out_point: OutPoint, sats_fee_amount: u64) -> Result<(), DbError>;
    async fn get_utxo_by_btc_address(&self, btc_address: String) -> Result<Option<Utxo>, DbError>;
}

#[async_trait]
impl UtxoStorage for LocalDbStorage {
    #[instrument(level = "trace", skip_all)]
    async fn insert_utxo(&self, utxo: Utxo) -> Result<Utxo, DbError> {
        let rec = sqlx::query_as::<_, UtxoRow>(
            r#"
            INSERT INTO gateway.utxo
                (out_point, rune_amount, rune_id, status, btc_address, sats_fee_amount)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (out_point) DO UPDATE
            SET rune_amount = EXCLUDED.rune_amount,
                rune_id = EXCLUDED.rune_id,
                status = EXCLUDED.status,
                btc_address = EXCLUDED.btc_address,
                sats_fee_amount = EXCLUDED.sats_fee_amount
            RETURNING out_point, rune_amount, rune_id, status, btc_address, sats_fee_amount
            "#,
        )
        .bind(utxo.out_point.to_string())
        .bind(utxo.rune_amount as i64)
        .bind(&utxo.rune_id)
        .bind(utxo.status)
        .bind(&utxo.btc_address.to_string())
        .bind(utxo.sats_fee_amount as i64)
        .fetch_one(&self.postgres_repo.pool)
        .await
        .map_err(|e| DbError::BadRequest(e.to_string()))?;

        Ok(Utxo::from_row(rec, self.network)?)
    }

    #[instrument(level = "trace", skip_all)]
    async fn update_status(&self, out_point: OutPoint, new_status: UtxoStatus) -> Result<(), DbError> {
        let rows = sqlx::query(
            r#"
            UPDATE gateway.utxo
            SET status = $1
            WHERE out_point = $2
            "#,
        )
        .bind(new_status)
        .bind(out_point.to_string())
        .execute(&self.postgres_repo.pool)
        .await
        .map_err(|e| DbError::BadRequest(e.to_string()))?
        .rows_affected();

        if rows == 0 {
            return Err(DbError::NotFound(format!("UTXO {out_point} not found")));
        }

        Ok(())
    }

    #[instrument(level = "trace", skip_all)]
    async fn list_unspent(&self, rune_id: String) -> Result<Vec<Utxo>, DbError> {
        let rows = sqlx::query_as::<_, UtxoRow>(
            r#"
            SELECT out_point, rune_amount, rune_id, status, btc_address, sats_fee_amount
            FROM gateway.utxo
            WHERE rune_id = $1 AND status IN ('confirmed', 'pending')
            ORDER BY
                CASE WHEN status = 'confirmed' THEN 0 ELSE 1 END,
                rune_amount ASC
            "#,
        )
        .bind(rune_id)
        .fetch_all(&self.postgres_repo.pool)
        .await
        .map_err(|e| DbError::BadRequest(e.to_string()))?;

        Ok(rows
            .into_iter()
            .map(|row| Utxo::from_row(row, self.network))
            .collect::<Result<Vec<Utxo>, DbError>>()?)
    }

    #[instrument(level = "trace", skip_all)]
    async fn select_utxos_for_amount(&self, rune_id: String, target_amount: u64) -> Result<Vec<Utxo>, DbError> {
        let mut tx = self
            .postgres_repo
            .pool
            .begin()
            .await
            .map_err(|e| DbError::BadRequest(format!("Failed to begin transaction: {}", e)))?;

        let candidates = get_candidate_utxos_for_update(rune_id, &mut tx, self.network).await?;

        let mut selected = Vec::new();
        let mut total = 0u64;
        for u in candidates {
            if total < target_amount {
                total += u.rune_amount;
                selected.push(u);
            } else {
                break;
            }
        }

        if total < target_amount {
            tx.rollback().await.ok();
            return Err(DbError::BadRequest("Not enough funds".into()));
        }

        let utxo_refs: Vec<String> = selected.iter().map(|u| u.out_point.to_string()).collect();

        let updated_utxos = mark_utxos_as_spent(&utxo_refs, &mut tx, self.network).await?;

        tx.commit()
            .await
            .map_err(|e| DbError::BadRequest(format!("Failed to commit transaction: {}", e)))?;

        Ok(updated_utxos)
    }

    #[instrument(level = "trace", skip_all)]
    async fn get_utxo(&self, out_point: OutPoint) -> Result<Option<Utxo>, DbError> {
        let utxo = sqlx::query_as::<_, UtxoRow>(
            r#"
            SELECT out_point, rune_amount, rune_id, status, btc_address, sats_fee_amount
            FROM gateway.utxo
            WHERE out_point = $1
            "#,
        )
        .bind(out_point.to_string())
        .fetch_optional(&self.postgres_repo.pool)
        .await
        .map_err(|e| DbError::BadRequest(e.to_string()))?;

        match utxo {
            Some(row) => Ok(Some(Utxo::from_row(row, self.network)?)),
            None => Ok(None),
        }
    }

    #[instrument(level = "trace", skip_all)]
    async fn delete_utxo(&self, out_point: OutPoint) -> Result<(), DbError> {
        let rows = sqlx::query(
            r#"
            DELETE FROM gateway.utxo
            WHERE out_point = $1
            "#,
        )
        .bind(out_point.to_string())
        .execute(&self.postgres_repo.pool)
        .await
        .map_err(|e| DbError::BadRequest(e.to_string()))?
        .rows_affected();

        if rows == 0 {
            return Err(DbError::NotFound(format!("UTXO {out_point} not found")));
        }

        Ok(())
    }

    #[instrument(level = "trace", skip_all)]
    async fn update_sats_fee_amount(&self, out_point: OutPoint, sats_fee_amount: u64) -> Result<(), DbError> {
        let rows = sqlx::query(
            r#"
            UPDATE gateway.utxo
            SET sats_fee_amount = $1
            WHERE out_point = $2
        "#,
        )
        .bind(sats_fee_amount as i64)
        .bind(out_point.to_string())
        .execute(&self.postgres_repo.pool)
        .await
        .map_err(|e| DbError::BadRequest(e.to_string()))?
        .rows_affected();

        if rows == 0 {
            return Err(DbError::NotFound(format!("UTXO {} not found", out_point)));
        }

        Ok(())
    }

    #[instrument(level = "trace", skip_all)]
    async fn get_utxo_by_btc_address(&self, btc_address: String) -> Result<Option<Utxo>, DbError> {
        let utxo = sqlx::query_as::<_, UtxoRow>(
            r#"
            SELECT out_point, rune_amount, rune_id, status, btc_address, sats_fee_amount
            FROM gateway.utxo
            WHERE btc_address = $1
        "#,
        )
        .bind(btc_address)
        .fetch_optional(&self.postgres_repo.pool)
        .await
        .map_err(|e| DbError::BadRequest(e.to_string()))?;

        match utxo {
            Some(row) => Ok(Some(Utxo::from_row(row, self.network)?)),
            None => Ok(None),
        }
    }
}

#[instrument(level = "trace", skip_all)]
async fn get_candidate_utxos_for_update(
    rune_id: String,
    tx: &mut Transaction<'_, Postgres>,
    network: bitcoin::Network,
) -> Result<Vec<Utxo>, DbError> {
    let candidates = sqlx::query_as::<_, UtxoRow>(
        r#"
        SELECT out_point, rune_amount, rune_id, status, btc_address, sats_fee_amount
        FROM gateway.utxo
        WHERE rune_id = $1 AND status IN ('confirmed', 'pending')
        ORDER BY rune_amount ASC
        FOR UPDATE SKIP LOCKED
        "#,
    )
    .bind(rune_id)
    .fetch_all(&mut **tx)
    .await
    .map_err(|e| DbError::BadRequest(e.to_string()))?;

    Ok(candidates
        .into_iter()
        .map(|row| Utxo::from_row(row, network))
        .collect::<Result<Vec<Utxo>, DbError>>()?)
}

#[instrument(level = "trace", skip_all)]
async fn mark_utxos_as_spent(
    utxo_refs: &[String],
    tx: &mut Transaction<'_, Postgres>,
    network: bitcoin::Network,
) -> Result<Vec<Utxo>, DbError> {
    if utxo_refs.is_empty() {
        return Ok(Vec::new());
    }

    let mut conditions = Vec::new();
    let mut bind_idx = 1;
    for _ in utxo_refs {
        conditions.push(format!("out_point = ${}", bind_idx));
        bind_idx += 2;
    }
    let where_clause = conditions.join(" OR ");

    let query_str = format!(
        r#"
        UPDATE gateway.utxo
        SET status = 'spent'
        WHERE {}
        RETURNING out_point, rune_amount, rune_id, status, btc_address, sats_fee_amount
        "#,
        where_clause
    );

    let mut query = sqlx::query_as::<_, UtxoRow>(&query_str);

    for out_point in utxo_refs {
        query = query.bind(out_point);
    }

    let updated_utxos = query
        .fetch_all(&mut **tx)
        .await
        .map_err(|e| DbError::BadRequest(e.to_string()))?;

    Ok(updated_utxos
        .into_iter()
        .map(|row| Utxo::from_row(row, network))
        .collect::<Result<Vec<Utxo>, DbError>>()?)
}
