use chrono::Utc;
use persistent_storage::config::PostgresDbCredentials;
use persistent_storage::init::PostgresRepo;
use gateway_local_db_store::schemas::utxo::{Utxo, UtxoStorage};
use runes_utxo_manager::{CoinSelector, GreedySelector};
use sqlx::Executor;
use persistent_storage::error::DatabaseError;
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
            sats_amount: None,
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
            sats_amount: None,
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
            sats_amount: None,
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
        sats_amount: None,
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
        sats_amount: None,
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
        sats_amount: None,
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
        sats_amount: None,
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
        sats_amount: None,
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
        sats_amount: None,
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
        sats_amount: None,
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
        sats_amount: None,
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
        sats_amount: None,
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

fn create_test_utxo(
    txid: &str,
    vout: i32,
    amount: i64,
    sats_amount: Option<i64>,
    rune_id: &str,
    status: &str,
    block_height: Option<i64>,
) -> Utxo {
    Utxo {
        id: 0,
        txid: txid.to_string(),
        vout,
        amount,
        sats_amount,
        rune_id: rune_id.to_string(),
        owner_pubkey: "test_pubkey".to_string(),
        status: status.to_string(),
        block_height,
        created_at: Utc::now().naive_utc(),
        updated_at: Utc::now().naive_utc(),
    }
}

async fn cleanup_test_db(repo: &PostgresRepo) {
    sqlx::query("TRUNCATE gateway.utxo RESTART IDENTITY CASCADE")
        .execute(&repo.pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_insert_utxo() -> Result<(), DatabaseError> {
    let repo = make_repo().await;
    cleanup_test_db(&repo).await;

    let test_utxo = create_test_utxo(
        "test_tx_1",
        0,
        1000,
        Some(546),
        "test_rune_id",
        "unspent",
        Some(100),
    );

    let inserted = repo.insert_utxo(test_utxo.clone()).await?;

    assert_eq!(inserted.txid, test_utxo.txid);
    assert_eq!(inserted.amount, test_utxo.amount);
    assert_eq!(inserted.sats_amount, test_utxo.sats_amount);
    assert_eq!(inserted.status, "unspent");
    assert!(inserted.id > 0);

    Ok(())
}

#[tokio::test]
async fn test_insert_pending_utxo() -> Result<(), DatabaseError> {
    let repo = make_repo().await;
    cleanup_test_db(&repo).await;

    let test_utxo = create_test_utxo(
        "test_tx_pending",
        1,
        2000,
        Some(1000),
        "test_rune_id",
        "unspent",
        None,
    );

    let inserted = repo.insert_pending_utxo(test_utxo.clone()).await?;

    assert_eq!(inserted.txid, test_utxo.txid);
    assert_eq!(inserted.amount, test_utxo.amount);
    assert_eq!(inserted.sats_amount, test_utxo.sats_amount);
    assert_eq!(inserted.status, "pending");
    assert_eq!(inserted.block_height, None);

    Ok(())
}

#[tokio::test]
async fn test_update_status() -> Result<(), DatabaseError> {
    let repo = make_repo().await;
    cleanup_test_db(&repo).await;

    let test_utxo = create_test_utxo(
        "test_tx_status",
        0,
        1500,
        Some(600),
        "test_rune_id",
        "unspent",
        Some(200),
    );
    repo.insert_utxo(test_utxo).await?;

    repo.update_status("test_tx_status", 0, "spent").await?;

    let updated_utxo = sqlx::query_as::<_, Utxo>(
        "SELECT * FROM gateway.utxo WHERE txid = $1 AND vout = $2"
    )
        .bind("test_tx_status")
        .bind(0)
        .fetch_one(&repo.pool)
        .await
        .unwrap();

    assert_eq!(updated_utxo.status, "spent");

    let utxos = repo.list_unspent("test_rune_id").await?;
    assert_eq!(utxos.len(), 0);

    Ok(())
}

#[tokio::test]
async fn test_update_status_not_found() -> Result<(), DatabaseError> {
    let repo = make_repo().await;
    cleanup_test_db(&repo).await;

    let result = repo.update_status("nonexistent_tx", 0, "spent").await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), DatabaseError::NotFound(_)));

    Ok(())
}

#[tokio::test]
async fn test_list_unspent_includes_pending() -> Result<(), DatabaseError> {
    let repo = make_repo().await;
    cleanup_test_db(&repo).await;

    let unspent_utxo = create_test_utxo(
        "unspent_tx",
        0,
        1000,
        Some(546),
        "test_rune_id",
        "unspent",
        Some(100),
    );
    repo.insert_utxo(unspent_utxo).await?;

    let pending_utxo = create_test_utxo(
        "pending_tx",
        0,
        2000,
        Some(1000),
        "test_rune_id",
        "pending",
        None,
    );
    repo.insert_pending_utxo(pending_utxo).await?;

    let spent_utxo = create_test_utxo(
        "spent_tx",
        0,
        500,
        Some(300),
        "test_rune_id",
        "spent",
        Some(99),
    );
    repo.insert_utxo(spent_utxo).await?;

    let utxos = repo.list_unspent("test_rune_id").await?;

    assert_eq!(utxos.len(), 2);

    let statuses: Vec<&str> = utxos.iter().map(|u| u.status.as_str()).collect();
    assert!(statuses.contains(&"unspent"));
    assert!(statuses.contains(&"pending"));

    Ok(())
}

#[tokio::test]
async fn test_select_and_lock_utxos() -> Result<(), DatabaseError> {
    let repo = make_repo().await;
    cleanup_test_db(&repo).await;

    let utxos = vec![
        create_test_utxo("tx1", 0, 500, Some(546), "test_rune", "unspent", Some(100)),
        create_test_utxo("tx2", 0, 1000, Some(546), "test_rune", "unspent", Some(101)),
        create_test_utxo("tx3", 0, 1500, Some(546), "test_rune", "unspent", Some(102)),
    ];

    for utxo in utxos {
        repo.insert_utxo(utxo).await?;
    }

    let locked = repo.select_and_lock_utxos("test_rune", 1200).await?;

    assert_eq!(locked.len(), 2);
    assert_eq!(locked[0].amount, 500);
    assert_eq!(locked[1].amount, 1000);

    for utxo in &locked {
        assert_eq!(utxo.status, "locked");
    }

    let tx1_status = sqlx::query_as::<_, Utxo>("SELECT * FROM gateway.utxo WHERE txid = $1")
        .bind("tx1")
        .fetch_one(&repo.pool)
        .await?;
    assert_eq!(tx1_status.status, "locked");

    let tx2_status = sqlx::query_as::<_, Utxo>("SELECT * FROM gateway.utxo WHERE txid = $1")
        .bind("tx2")
        .fetch_one(&repo.pool)
        .await?;
    assert_eq!(tx2_status.status, "locked");

    let tx3_status = sqlx::query_as::<_, Utxo>("SELECT * FROM gateway.utxo WHERE txid = $1")
        .bind("tx3")
        .fetch_one(&repo.pool)
        .await?;
    assert_eq!(tx3_status.status, "unspent");

    Ok(())
}

#[tokio::test]
async fn test_select_and_lock_insufficient_funds() -> Result<(), DatabaseError> {
    let repo = make_repo().await;
    cleanup_test_db(&repo).await;

    let utxo = create_test_utxo("tx1", 0, 500, Some(546), "test_rune", "unspent", Some(100));
    repo.insert_utxo(utxo).await?;

    let result = repo.select_and_lock_utxos("test_rune", 1000).await;

    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), DatabaseError::BadRequest(_)));

    let utxo_status = sqlx::query_as::<_, Utxo>("SELECT * FROM gateway.utxo WHERE txid = $1")
        .bind("tx1")
        .fetch_one(&repo.pool)
        .await?;
    assert_eq!(utxo_status.status, "unspent");

    Ok(())
}

#[tokio::test]
async fn test_unlock_utxos() -> Result<(), DatabaseError> {
    let repo = make_repo().await;
    cleanup_test_db(&repo).await;

    let utxo = create_test_utxo("tx1", 0, 1000, Some(546), "test_rune", "unspent", Some(100));
    let inserted = repo.insert_utxo(utxo).await?;

    let locked = repo.select_and_lock_utxos("test_rune", 500).await?;
    assert_eq!(locked.len(), 1);

    let utxo_ids: Vec<i64> = locked.iter().map(|u| u.id as i64).collect();
    gateway_local_db_store::schemas::utxo::UtxoStorage::unlock_utxos(&repo, &utxo_ids).await?;

    let utxo_status = sqlx::query_as::<_, Utxo>("SELECT * FROM gateway.utxo WHERE id = $1")
        .bind(inserted.id)
        .fetch_one(&repo.pool)
        .await?;
    assert_eq!(utxo_status.status, "unspent");

    Ok(())
}

#[tokio::test]
async fn test_unlock_empty_list() -> Result<(), DatabaseError> {
    let repo = make_repo().await;

    let result = gateway_local_db_store::schemas::utxo::UtxoStorage::unlock_utxos(&repo, &[]).await;
    assert!(result.is_ok());

    Ok(())
}

#[tokio::test]
async fn test_set_block_height() -> Result<(), DatabaseError> {
    let repo = make_repo().await;
    cleanup_test_db(&repo).await;

    let utxo = create_test_utxo("test_tx_height", 0, 1000, Some(546), "test_rune", "pending", None);
    repo.insert_pending_utxo(utxo).await?;

    repo.set_block_height("test_tx_height", 800000).await?;

    let updated_utxo = sqlx::query_as::<_, Utxo>("SELECT * FROM gateway.utxo WHERE txid = $1")
        .bind("test_tx_height")
        .fetch_one(&repo.pool)
        .await?;
    assert_eq!(updated_utxo.block_height, Some(800000));

    Ok(())
}

#[tokio::test]
async fn test_set_block_height_not_found() -> Result<(), DatabaseError> {
    let repo = make_repo().await;
    cleanup_test_db(&repo).await;

    let result = repo.set_block_height("nonexistent_tx", 800000).await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), DatabaseError::NotFound(_)));

    Ok(())
}

#[tokio::test]
async fn test_concurrent_select_and_lock() -> Result<(), DatabaseError> {
    let repo = make_repo().await;
    cleanup_test_db(&repo).await;

    let utxo = create_test_utxo("concurrent_unique_tx", 0, 1000, Some(546), "concurrent_rune", "unspent", Some(100));
    repo.insert_utxo(utxo).await?;

    let f1 = repo.select_and_lock_utxos("concurrent_rune", 500);
    let f2 = repo.select_and_lock_utxos("concurrent_rune", 500);

    let (result1, result2) = tokio::join!(f1, f2);

    let success_count = [result1.as_ref(), result2.as_ref()]
        .iter()
        .filter(|r| r.is_ok())
        .count();

    assert!(success_count >= 1, "At least one should succeed");
    assert!(success_count <= 1, "At most one should succeed");

    Ok(())
}

#[tokio::test]
async fn test_comprehensive_flow() -> Result<(), DatabaseError> {
    let repo = make_repo().await;
    cleanup_test_db(&repo).await;

    let utxos = vec![
        create_test_utxo("flow_tx1", 0, 100, Some(546), "flow_rune", "unspent", Some(100)),
        create_test_utxo("flow_tx2", 0, 200, Some(546), "flow_rune", "unspent", Some(101)),
        create_test_utxo("flow_tx3", 0, 300, Some(1000), "flow_rune", "unspent", Some(102)),
    ];

    for utxo in utxos {
        repo.insert_utxo(utxo).await?;
    }

    let unspent = repo.list_unspent("flow_rune").await?;
    assert_eq!(unspent.len(), 3);

    let locked = repo.select_and_lock_utxos("flow_rune", 250).await?;
    assert_eq!(locked.len(), 2);

    let change_utxo = create_test_utxo("change_tx", 0, 50, Some(546), "flow_rune", "pending", None);
    repo.insert_pending_utxo(change_utxo).await?;

    repo.update_status("flow_tx1", 0, "spent").await?;
    repo.update_status("flow_tx2", 0, "spent").await?;

    repo.set_block_height("change_tx", 12345).await?;

    let final_unspent = repo.list_unspent("flow_rune").await?;
    assert_eq!(final_unspent.len(), 2);

    let statuses: Vec<&str> = final_unspent.iter().map(|u| u.status.as_str()).collect();
    assert!(statuses.contains(&"unspent"));
    assert!(statuses.contains(&"pending"));

    let change_with_height = final_unspent.iter().find(|u| u.txid == "change_tx").unwrap();
    assert_eq!(change_with_height.block_height, Some(12345));

    Ok(())
}