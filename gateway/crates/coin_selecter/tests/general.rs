use chrono::{Utc, Duration};
use persistent_storage::config::PostgresDbCredentials;
use persistent_storage::init::PostgresRepo;
use gateway_local_db_store::schemas::utxo::*;
use gateway_local_db_store::errors::*;
use runes_utxo_manager::{CoinSelector, GreedySelector};

#[tokio::test]
async fn test_utxo_flow() -> Result<(), DatabaseError> {
    let url = "postgresql://admin_manager:password@localhost:5432/postgres".to_string();

    let repo = PostgresRepo::from_config(PostgresDbCredentials { url }).await.unwrap();

    sqlx::query("DELETE FROM gateway.utxo").execute(&repo.pool).await.unwrap();

    let utxos = vec![
        Utxo {
            id: 0,
            txid: "tx1".into(),
            vout: 0,
            amount: 30,
            rune_id: "rune1".into(),
            owner_address: "addr1".into(),
            nonce: None,
            status: "unspent".into(),
            block_height: Some(100),
            lock_expires_at: Some(Utc::now() + Duration::minutes(10)),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        },
        Utxo {
            id: 0,
            txid: "tx2".into(),
            vout: 1,
            amount: 50,
            rune_id: "rune1".into(),
            owner_address: "addr1".into(),
            nonce: None,
            status: "unspent".into(),
            block_height: Some(101),
            lock_expires_at: Some(Utc::now() + Duration::minutes(10)),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        },
        Utxo {
            id: 0,
            txid: "tx3".into(),
            vout: 0,
            amount: 20,
            rune_id: "rune2".into(),
            owner_address: "addr2".into(),
            nonce: None,
            status: "unspent".into(),
            block_height: Some(102),
            lock_expires_at: Some(Utc::now() + Duration::minutes(10)),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        },
    ];

    for u in utxos.iter() {
        repo.insert_utxo(u.clone()).await?;
    }

    let unspent_rune1 = repo.list_unspent("rune1").await?;
    assert_eq!(unspent_rune1.len(), 2);
    assert!(unspent_rune1.iter().all(|u| u.status == "unspent"));

    let selector = GreedySelector { repo: &repo };
    let selected = selector.select_utxos("rune1", 70).await?;
    assert_eq!(selected.len(), 2); // должно выбрать tx1 + tx2
    assert_eq!(selected.iter().map(|u| u.amount).sum::<i64>(), 80);

    for u in selected.iter() {
        let utxo = sqlx::query_as::<_, Utxo>(
            "SELECT * FROM gateway.utxo WHERE txid = $1 AND vout = $2"
        )
            .bind(&u.txid)
            .bind(u.vout)
            .fetch_one(&repo.pool)
            .await;
        assert_eq!(utxo.unwrap().status, "locked");
    }

    let res = selector.select_utxos("rune1", 1000).await;
    assert!(res.is_err());

    Ok(())
}
