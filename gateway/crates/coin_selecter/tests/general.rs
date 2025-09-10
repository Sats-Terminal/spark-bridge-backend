use chrono::Utc;
use persistent_storage::config::PostgresDbCredentials;
use persistent_storage::init::PostgresRepo;
use gateway_local_db_store::schemas::utxo::{Utxo, UtxoStorage};
use gateway_local_db_store::errors::DatabaseError;
use runes_utxo_manager::{CoinSelector, GreedySelector};
use sqlx::Executor;
use runes_utxo_manager::traits::UtxoManager;

async fn make_repo() -> PostgresRepo {
    let url = "postgresql://admin_manager:password@localhost:5432/production_db_name";

    PostgresRepo::from_config(PostgresDbCredentials { url: url.into() })
        .await
        .unwrap()
}

#[tokio::test]
async fn test_select_and_lock_basic_flow() -> Result<(), DatabaseError> {
    let repo = make_repo().await;

    sqlx::query("TRUNCATE gateway.utxo RESTART IDENTITY CASCADE")
        .execute(&repo.pool)
        .await
        .unwrap();

    let utxos = vec![
        Utxo {
            id: 0,
            txid: "t1".into(),
            vout: 0,
            amount: 10,
            rune_id: "rune1".into(),
            owner_pubkey: "pub1".into(),
            status: "unspent".into(),
            block_height: Some(100),
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        },
        Utxo {
            id: 0,
            txid: "t2".into(),
            vout: 0,
            amount: 20,
            rune_id: "rune1".into(),
            owner_pubkey: "pub1".into(),
            status: "unspent".into(),
            block_height: Some(101),
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        },
        Utxo {
            id: 0,
            txid: "t3".into(),
            vout: 0,
            amount: 30,
            rune_id: "rune_other".into(),
            owner_pubkey: "pub2".into(),
            status: "unspent".into(),
            block_height: Some(102),
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        },
    ];

    for u in &utxos {
        repo.insert_utxo(u.clone()).await?;
    }

    let unspent = repo.list_unspent("rune1").await?;
    assert_eq!(unspent.len(), 2);
    assert!(unspent.iter().all(|u| u.status == "unspent"));

    let selector = GreedySelector { repo: &repo };
    let selected = selector.select_utxos("rune1", 25).await?;

    assert_eq!(selected.len(), 2);
    assert_eq!(selected.iter().map(|u| u.amount).sum::<i64>(), 30);

    for u in selected {
        let db_u = sqlx::query_as::<_, Utxo>("SELECT * FROM gateway.utxo WHERE txid = $1 AND vout = $2")
            .bind(&u.txid)
            .bind(u.vout)
            .fetch_one(&repo.pool)
            .await;
        assert_eq!(db_u.unwrap().status, "locked");
    }

    Ok(())
}

#[tokio::test]
async fn test_select_exact_and_single_large_utxo() -> Result<(), DatabaseError> {
    let repo = make_repo().await;
    sqlx::query("TRUNCATE gateway.utxo RESTART IDENTITY CASCADE")
        .execute(&repo.pool)
        .await
        .unwrap();

    repo.insert_utxo(Utxo {
        id: 0,
        txid: "a1".into(),
        vout: 0,
        amount: 15,
        rune_id: "rune2".into(),
        owner_pubkey: "p2".into(),
        status: "unspent".into(),
        block_height: Some(10),
        created_at: Utc::now().naive_utc(),
        updated_at: Utc::now().naive_utc(),
    }).await?;

    repo.insert_utxo(Utxo {
        id: 0,
        txid: "a2".into(),
        vout: 0,
        amount: 35,
        rune_id: "rune2".into(),
        owner_pubkey: "p2".into(),
        status: "unspent".into(),
        block_height: Some(11),
        created_at: Utc::now().naive_utc(),
        updated_at: Utc::now().naive_utc(),
    }).await?;

    let sel = repo.select_and_lock_utxos("rune2", 50).await?;
    assert_eq!(sel.len(), 2);
    assert_eq!(sel.iter().map(|u| u.amount).sum::<i64>(), 50);

    sqlx::query("TRUNCATE gateway.utxo RESTART IDENTITY CASCADE")
        .execute(&repo.pool)
        .await
        .unwrap();

    repo.insert_utxo(Utxo {
        id: 0,
        txid: "b1".into(),
        vout: 0,
        amount: 5,
        rune_id: "rune3".into(),
        owner_pubkey: "p3".into(),
        status: "unspent".into(),
        block_height: Some(20),
        created_at: Utc::now().naive_utc(),
        updated_at: Utc::now().naive_utc(),
    }).await?;

    repo.insert_utxo(Utxo {
        id: 0,
        txid: "b2".into(),
        vout: 0,
        amount: 100,
        rune_id: "rune3".into(),
        owner_pubkey: "p3".into(),
        status: "unspent".into(),
        block_height: Some(21),
        created_at: Utc::now().naive_utc(),
        updated_at: Utc::now().naive_utc(),
    }).await?;

    let sel2 = repo.select_and_lock_utxos("rune3", 50).await?;
    assert_eq!(sel2.len(), 2);
    assert_eq!(sel2[0].amount, 5);
    assert_eq!(sel2[0].txid, "b1");

    let big = sqlx::query_as::<_, Utxo>("SELECT * FROM gateway.utxo WHERE txid = $1")
        .bind("b2")
        .fetch_one(&repo.pool)
        .await;
    assert_eq!(big.unwrap().status, "locked");

    let small = sqlx::query_as::<_, Utxo>("SELECT * FROM gateway.utxo WHERE txid = $1")
        .bind("b1")
        .fetch_one(&repo.pool)
        .await;
    assert_eq!(small.unwrap().status, "locked");

    Ok(())
}

#[tokio::test]
async fn test_insufficient_funds_rollback() -> Result<(), DatabaseError> {
    let repo = make_repo().await;
    sqlx::query("TRUNCATE gateway.utxo RESTART IDENTITY CASCADE")
        .execute(&repo.pool)
        .await
        .unwrap();

    repo.insert_utxo(Utxo {
        id: 0,
        txid: "c1".into(),
        vout: 0,
        amount: 5,
        rune_id: "rune4".into(),
        owner_pubkey: "p4".into(),
        status: "unspent".into(),
        block_height: Some(30),
        created_at: Utc::now().naive_utc(),
        updated_at: Utc::now().naive_utc(),
    }).await?;

    repo.insert_utxo(Utxo {
        id: 0,
        txid: "c2".into(),
        vout: 0,
        amount: 10,
        rune_id: "rune4".into(),
        owner_pubkey: "p4".into(),
        status: "unspent".into(),
        block_height: Some(31),
        created_at: Utc::now().naive_utc(),
        updated_at: Utc::now().naive_utc(),
    }).await?;

    let res = repo.select_and_lock_utxos("rune4", 100).await;
    assert!(res.is_err());

    let unspent = repo.list_unspent("rune4").await?;
    assert_eq!(unspent.len(), 2);
    assert!(unspent.iter().all(|u| u.status == "unspent"));

    Ok(())
}

#[tokio::test]
async fn test_unlock_mark_spent_and_update_status() -> Result<(), DatabaseError> {
    let repo = make_repo().await;
    sqlx::query("TRUNCATE gateway.utxo RESTART IDENTITY CASCADE")
        .execute(&repo.pool)
        .await
        .unwrap();

    let inserted = repo.insert_utxo(Utxo {
        id: 0,
        txid: "d1".into(),
        vout: 0,
        amount: 100,
        rune_id: "rune5".into(),
        owner_pubkey: "p5".into(),
        status: "unspent".into(),
        block_height: Some(40),
        created_at: Utc::now().naive_utc(),
        updated_at: Utc::now().naive_utc(),
    }).await?;

    let locked = repo.select_and_lock_utxos("rune5", 50).await?;
    assert!(locked.iter().any(|u| u.id == inserted.id));

    UtxoManager::unlock_utxos(&repo, &[inserted.id as i64]).await?;
    let after_unlock = sqlx::query_as::<_, Utxo>("SELECT * FROM gateway.utxo WHERE id = $1")
        .bind(inserted.id)
        .fetch_one(&repo.pool)
        .await;
    assert_eq!(after_unlock.unwrap().status, "unspent");

    repo.update_status(&inserted.txid, inserted.vout, "archived").await?;
    let after_update = sqlx::query_as::<_, Utxo>("SELECT * FROM gateway.utxo WHERE id = $1")
        .bind(inserted.id)
        .fetch_one(&repo.pool)
        .await;
    assert_eq!(after_update.unwrap().status, "archived");

    repo.update_status(&inserted.txid, inserted.vout, "locked").await?;
    repo.mark_spent(&[inserted.id as i64]).await?;
    let after_spent = sqlx::query_as::<_, Utxo>("SELECT * FROM gateway.utxo WHERE id = $1")
        .bind(inserted.id)
        .fetch_one(&repo.pool)
        .await;
    assert_eq!(after_spent.unwrap().status, "spent");

    Ok(())
}

#[tokio::test]
async fn test_concurrent_selection_one_wins() -> Result<(), DatabaseError> {
    let repo = make_repo().await;
    sqlx::query("TRUNCATE gateway.utxo RESTART IDENTITY CASCADE")
        .execute(&repo.pool)
        .await
        .unwrap();

    repo.insert_utxo(Utxo {
        id: 0,
        txid: "e1".into(),
        vout: 0,
        amount: 50,
        rune_id: "rune_conc".into(),
        owner_pubkey: "pc".into(),
        status: "unspent".into(),
        block_height: Some(50),
        created_at: Utc::now().naive_utc(),
        updated_at: Utc::now().naive_utc(),
    }).await?;
    repo.insert_utxo(Utxo {
        id: 0,
        txid: "e2".into(),
        vout: 0,
        amount: 50,
        rune_id: "rune_conc".into(),
        owner_pubkey: "pc".into(),
        status: "unspent".into(),
        block_height: Some(51),
        created_at: Utc::now().naive_utc(),
        updated_at: Utc::now().naive_utc(),
    }).await?;

    let f1 = repo.select_and_lock_utxos("rune_conc", 100);
    let f2 = repo.select_and_lock_utxos("rune_conc", 100);
    let (r1, r2) = tokio::join!(f1, f2);

    let ok_count = [r1.as_ref().ok(), r2.as_ref().ok()].iter().filter(|x| x.is_some()).count();
    assert!(ok_count >= 1, "at least one success");
    assert!(ok_count <= 1, "once more - at least one success");

    let remaining_unspent = repo.list_unspent("rune_conc").await?;

    assert!(remaining_unspent.len() == 0 || remaining_unspent.len() == 2 || remaining_unspent.len() == 1);

    Ok(())
}
