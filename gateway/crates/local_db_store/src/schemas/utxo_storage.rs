use crate::storage::LocalDbStorage;
use async_trait::async_trait;
use bitcoin::OutPoint;
use global_utils::conversion::decode_address;
use persistent_storage::error::DbError;
use serde::{Deserialize, Serialize};
use sqlx::{Postgres, Transaction};

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

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use std::sync::Arc;
//     use persistent_storage::error::DbError as DatabaseError;
//
//     async fn make_repo(db: sqlx::PgPool) -> Arc<LocalDbStorage> {
//         Arc::new(LocalDbStorage {
//             postgres_repo: crate::storage::PostgresRepo { pool: db },
//         })
//     }
//
//     pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");
//
//     async fn cleanup_and_setup(repo: &LocalDbStorage) {
//         sqlx::query("TRUNCATE gateway.utxo RESTART IDENTITY CASCADE")
//             .execute(&repo.postgres_repo.pool)
//             .await
//             .unwrap();
//     }
//
//     fn create_test_utxo(txid: &str, vout: u32, amount: u64, sats_fee_amount: u64) -> Utxo {
//         Utxo {
//             txid: txid.to_string(),
//             vout,
//             btc_address: "bc1qtest".to_string(),
//             transaction: None,
//             amount,
//             rune_id: "test_rune".to_string(),
//             status: UtxoStatus::Pending,
//             sats_fee_amount,
//         }
//     }
//
//     #[sqlx::test(migrator = "MIGRATOR")]
//     async fn test_insert_utxo(db: sqlx::PgPool) -> Result<(), DatabaseError> {
//         let repo = make_repo(db).await;
//         cleanup_and_setup(&repo).await;
//
//         let utxo = create_test_utxo("test_txid_1", 0, 1000, 100);
//
//         let result = repo.insert_utxo(utxo.clone()).await;
//         assert!(result.is_ok());
//
//         let inserted = result.unwrap();
//         assert_eq!(inserted.txid, utxo.txid);
//         assert_eq!(inserted.vout, utxo.vout);
//         assert_eq!(inserted.amount, utxo.amount);
//         assert_eq!(inserted.rune_id, utxo.rune_id);
//         assert_eq!(inserted.status, utxo.status);
//         assert_eq!(inserted.sats_fee_amount, utxo.sats_fee_amount);
//
//         Ok(())
//     }
//
//     #[sqlx::test(migrator = "MIGRATOR")]
//     async fn test_insert_utxo_conflict(db: sqlx::PgPool) -> Result<(), DatabaseError> {
//         let repo = make_repo(db).await;
//         cleanup_and_setup(&repo).await;
//
//         let utxo1 = create_test_utxo("test_txid_2", 0, 1000, 100);
//         let mut utxo2 = utxo1.clone();
//         utxo2.amount = 2000;
//         utxo2.sats_fee_amount = 200;
//         utxo2.status = UtxoStatus::Confirmed;
//
//         repo.insert_utxo(utxo1).await.unwrap();
//
//         let result = repo.insert_utxo(utxo2).await.unwrap();
//
//         assert_eq!(result.amount, 2000);
//         assert_eq!(result.sats_fee_amount, 200);
//         assert_eq!(result.status, UtxoStatus::Confirmed);
//
//         Ok(())
//     }
//
//     #[sqlx::test(migrator = "MIGRATOR")]
//     async fn test_get_utxo(db: sqlx::PgPool) -> Result<(), DatabaseError> {
//         let repo = make_repo(db).await;
//         cleanup_and_setup(&repo).await;
//
//         let utxo = create_test_utxo("test_txid_3", 0, 1000, 100);
//
//         repo.insert_utxo(utxo.clone()).await.unwrap();
//
//         let result = repo.get_utxo("test_txid_3", 0).await.unwrap();
//         assert!(result.is_some());
//
//         let found_utxo = result.unwrap();
//         assert_eq!(found_utxo.txid, utxo.txid);
//         assert_eq!(found_utxo.vout, utxo.vout);
//
//         let not_found = repo.get_utxo("nonexistent", 0).await.unwrap();
//         assert!(not_found.is_none());
//
//         Ok(())
//     }
//
//     #[sqlx::test(migrator = "MIGRATOR")]
//     async fn test_update_status(db: sqlx::PgPool) -> Result<(), DatabaseError> {
//         let repo = make_repo(db).await;
//         cleanup_and_setup(&repo).await;
//
//         let utxo = create_test_utxo("test_txid_4", 0, 1000, 100);
//
//         repo.insert_utxo(utxo).await.unwrap();
//
//         let result = repo
//             .update_status("test_txid_4", 0, UtxoStatus::Confirmed, None)
//             .await;
//         assert!(result.is_ok());
//
//         let updated_utxo = repo.get_utxo("test_txid_4", 0).await.unwrap().unwrap();
//         assert_eq!(updated_utxo.status, UtxoStatus::Confirmed);
//
//         let not_found_result = repo
//             .update_status("nonexistent", 0, UtxoStatus::Spent, None)
//             .await;
//         assert!(not_found_result.is_err());
//
//         Ok(())
//     }
//
//     #[sqlx::test(migrator = "MIGRATOR")]
//     async fn test_list_unspent(db: sqlx::PgPool) -> Result<(), DatabaseError> {
//         let repo = make_repo(db).await;
//         cleanup_and_setup(&repo).await;
//
//         let utxo1 = Utxo {
//             txid: "tx1".to_string(),
//             vout: 0,
//             btc_address: "bc1qtest".to_string(),
//             transaction: None,
//             amount: 1000,
//             rune_id: "test_rune".to_string(),
//             status: UtxoStatus::Confirmed,
//             sats_fee_amount: 100,
//         };
//
//         let utxo2 = Utxo {
//             txid: "tx2".to_string(),
//             vout: 0,
//             btc_address: "bc1qtest".to_string(),
//             transaction: None,
//             amount: 500,
//             rune_id: "test_rune".to_string(),
//             status: UtxoStatus::Pending,
//             sats_fee_amount: 50,
//         };
//
//         let utxo3 = Utxo {
//             txid: "tx3".to_string(),
//             vout: 0,
//             btc_address: "bc1qtest".to_string(),
//             transaction: None,
//             amount: 2000,
//             rune_id: "test_rune".to_string(),
//             status: UtxoStatus::Spent,
//             sats_fee_amount: 200,
//         };
//
//         repo.insert_utxo(utxo1).await.unwrap();
//         repo.insert_utxo(utxo2).await.unwrap();
//         repo.insert_utxo(utxo3).await.unwrap();
//
//         let unspent = repo.list_unspent("test_rune").await.unwrap();
//
//         assert_eq!(unspent.len(), 2);
//         assert!(!unspent.iter().any(|u| u.txid == "tx3"));
//
//         Ok(())
//     }
//
//     #[sqlx::test(migrator = "MIGRATOR")]
//     async fn test_select_utxos_for_amount(db: sqlx::PgPool) -> Result<(), DatabaseError> {
//         let repo = make_repo(db).await;
//         cleanup_and_setup(&repo).await;
//
//         let utxo1 = Utxo {
//             txid: "tx1".to_string(),
//             vout: 0,
//             btc_address: "bc1qtest".to_string(),
//             transaction: None,
//             amount: 1000,
//             rune_id: "test_rune".to_string(),
//             status: UtxoStatus::Confirmed,
//             sats_fee_amount: 100,
//         };
//
//         let utxo2 = Utxo {
//             txid: "tx2".to_string(),
//             vout: 0,
//             btc_address: "bc1qtest".to_string(),
//             transaction: None,
//             amount: 1500,
//             rune_id: "test_rune".to_string(),
//             status: UtxoStatus::Confirmed,
//             sats_fee_amount: 150,
//         };
//
//         repo.insert_utxo(utxo1).await.unwrap();
//         repo.insert_utxo(utxo2).await.unwrap();
//
//         let selected = repo
//             .select_utxos_for_amount("test_rune", 2000)
//             .await
//             .unwrap();
//
//         assert_eq!(selected.len(), 2);
//         assert!(selected.iter().all(|u| u.status == UtxoStatus::Spent));
//
//         let utxo1_after = repo.get_utxo("tx1", 0).await.unwrap().unwrap();
//         let utxo2_after = repo.get_utxo("tx2", 0).await.unwrap().unwrap();
//
//         assert_eq!(utxo1_after.status, UtxoStatus::Spent);
//         assert_eq!(utxo2_after.status, UtxoStatus::Spent);
//
//         Ok(())
//     }
//
//     #[sqlx::test(migrator = "MIGRATOR")]
//     async fn test_select_utxos_insufficient_funds(db: sqlx::PgPool) -> Result<(), DatabaseError> {
//         let repo = make_repo(db).await;
//         cleanup_and_setup(&repo).await;
//
//         let utxo = create_test_utxo("tx1", 0, 1000, 100);
//         repo.insert_utxo(utxo).await.unwrap();
//
//         let result = repo
//             .select_utxos_for_amount("test_rune", 2000)
//             .await;
//
//         assert!(result.is_err());
//
//         let utxo_after = repo.get_utxo("tx1", 0).await.unwrap().unwrap();
//         assert_eq!(utxo_after.status, UtxoStatus::Pending);
//
//         Ok(())
//     }
//
//     #[sqlx::test(migrator = "MIGRATOR")]
//     async fn test_delete_utxo(db: sqlx::PgPool) -> Result<(), DatabaseError> {
//         let repo = make_repo(db).await;
//         cleanup_and_setup(&repo).await;
//
//         let utxo = create_test_utxo("test_txid_5", 0, 1000, 100);
//
//         repo.insert_utxo(utxo).await.unwrap();
//
//         let result = repo.delete_utxo("test_txid_5", 0).await;
//         assert!(result.is_ok());
//
//         let not_found = repo.get_utxo("test_txid_5", 0).await.unwrap();
//         assert!(not_found.is_none());
//
//         let not_found_result = repo.delete_utxo("nonexistent", 0).await;
//         assert!(not_found_result.is_err());
//
//         Ok(())
//     }
//
//     #[test]
//     fn test_request_status_enum() {
//         let pending = RequestStatus::Pending;
//         let processing = RequestStatus::Processing;
//         let completed = RequestStatus::Completed;
//         let failed = RequestStatus::Failed;
//         let cancelled = RequestStatus::Cancelled;
//
//         assert_eq!(pending, RequestStatus::Pending);
//         assert_ne!(pending, processing);
//
//         let json = serde_json::to_string(&pending).unwrap();
//         assert_eq!(json, "\"Pending\"");
//
//         let deserialized: RequestStatus = serde_json::from_str(&json).unwrap();
//         assert_eq!(deserialized, RequestStatus::Pending);
//     }
// }
