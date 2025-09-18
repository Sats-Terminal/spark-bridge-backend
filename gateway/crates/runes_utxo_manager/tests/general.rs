// use chrono::Utc;
// use gateway_local_db_store::storage::LocalDbStorage;
// use persistent_storage::error::DbError;
// use runes_utxo_manager::greedy::{Utxo, UtxoStorage};
// use runes_utxo_manager::session_tracker::{RequestType, SessionStatus, SessionStorage, SessionTracker};
// use runes_utxo_manager::traits::UtxoManager;
// use runes_utxo_manager::{CoinSelector, GreedySelector};
// use serde_json::json;
// use sqlx::Executor;
// use uuid::Uuid;
//
// async fn make_repo() -> LocalDbStorage {
//     let url = "postgresql://admin_manager:password@localhost:5432/production_db_name";
//
//     LocalDbStorage::new(url.into()).await.unwrap()
// }
//
// #[tokio::test]
// async fn test_select_and_lock_basic_flow() -> Result<(), DbError> {
//     let repo = make_repo().await;
//
//     sqlx::query("TRUNCATE gateway.utxo RESTART IDENTITY CASCADE")
//         .execute(&repo.postgres_repo.pool)
//         .await
//         .unwrap();
// unwrap
//     let utxos = vec![
//         Utxo {
//             id: 0,
//             txid: "t1".into(),
//             vout: 0,
//             amount: 10,
//             rune_id: "rune1".into(),
//             sats_amount: None,
//             owner_pubkey: "pub1".into(),
//             status: "unspent".into(),
//             block_height: Some(100),
//             created_at: Utc::now().naive_utc(),
//             updated_at: Utc::now().naive_utc(),
//         },
//         Utxo {
//             id: 0,
//             txid: "t2".into(),
//             vout: 0,
//             amount: 20,
//             rune_id: "rune1".into(),
//             sats_amount: None,
//             owner_pubkey: "pub1".into(),
//             status: "unspent".into(),
//             block_height: Some(101),
//             created_at: Utc::now().naive_utc(),
//             updated_at: Utc::now().naive_utc(),
//         },
//         Utxo {
//             id: 0,
//             txid: "t3".into(),
//             vout: 0,
//             amount: 30,
//             rune_id: "rune_other".into(),
//             sats_amount: None,
//             owner_pubkey: "pub2".into(),
//             status: "unspent".into(),
//             block_height: Some(102),
//             created_at: Utc::now().naive_utc(),
//             updated_at: Utc::now().naive_utc(),
//         },
//     ];
//
//     for u in &utxos {
//         repo.insert_utxo(u.clone()).await?;
//     }
//
//     let unspent = repo.list_unspent("rune1").await?;
//     assert_eq!(unspent.len(), 2);
//     assert!(unspent.iter().all(|u| u.status == "unspent"));
//
//     let selector = GreedySelector { repo: &repo };
//     let selected = selector.select_utxos("rune1", 25).await?;
//
//     assert_eq!(selected.len(), 2);
//     assert_eq!(selected.iter().map(|u| u.amount).sum::<i64>(), 30);
//
//     for u in selected {
//         let db_u = sqlx::query_as::<_, Utxo>("SELECT * FROM gateway.utxo WHERE txid = $1 AND vout = $2")
//             .bind(&u.txid)
//             .bind(u.vout)
//             .fetch_one(&repo.postgres_repo.pool)
//             .await;
//         assert_eq!(db_u?.status, "locked");
//     }
//
//     Ok(())
// }
//
// #[tokio::test]
// async fn test_select_exact_and_single_large_utxo() -> Result<(), DbError> {
//     let repo = make_repo().await;
//     sqlx::query("TRUNCATE gateway.utxo RESTART IDENTITY CASCADE")
//         .execute(&repo.postgres_repo.pool)
//         .await?;
//
//     repo.insert_utxo(Utxo {
//         id: 0,
//         txid: "a1".into(),
//         vout: 0,
//         amount: 15,
//         rune_id: "rune2".into(),
//         sats_amount: None,
//         owner_pubkey: "p2".into(),
//         status: "unspent".into(),
//         block_height: Some(10),
//         created_at: Utc::now().naive_utc(),
//         updated_at: Utc::now().naive_utc(),
//     })
//     .await?;
//
//     repo.insert_utxo(Utxo {
//         id: 0,
//         txid: "a2".into(),
//         vout: 0,
//         amount: 35,
//         rune_id: "rune2".into(),
//         sats_amount: None,
//         owner_pubkey: "p2".into(),
//         status: "unspent".into(),
//         block_height: Some(11),
//         created_at: Utc::now().naive_utc(),
//         updated_at: Utc::now().naive_utc(),
//     })
//     .await?;
//
//     let sel = repo.select_and_lock_utxos("rune2", 50).await?;
//     assert_eq!(sel.len(), 2);
//     assert_eq!(sel.iter().map(|u| u.amount).sum::<i64>(), 50);
//
//     sqlx::query("TRUNCATE gateway.utxo RESTART IDENTITY CASCADE")
//         .execute(&repo.postgres_repo.pool)
//         .await?;
//
//     repo.insert_utxo(Utxo {
//         id: 0,
//         txid: "b1".into(),
//         vout: 0,
//         amount: 5,
//         rune_id: "rune3".into(),
//         sats_amount: None,
//         owner_pubkey: "p3".into(),
//         status: "unspent".into(),
//         block_height: Some(20),
//         created_at: Utc::now().naive_utc(),
//         updated_at: Utc::now().naive_utc(),
//     })
//     .await?;
//
//     repo.insert_utxo(Utxo {
//         id: 0,
//         txid: "b2".into(),
//         vout: 0,
//         amount: 100,
//         rune_id: "rune3".into(),
//         sats_amount: None,
//         owner_pubkey: "p3".into(),
//         status: "unspent".into(),
//         block_height: Some(21),
//         created_at: Utc::now().naive_utc(),
//         updated_at: Utc::now().naive_utc(),
//     })
//     .await?;
//
//     let sel2 = repo.select_and_lock_utxos("rune3", 50).await?;
//     assert_eq!(sel2.len(), 2);
//     assert_eq!(sel2[0].amount, 5);
//     assert_eq!(sel2[0].txid, "b1");
//
//     let big = sqlx::query_as::<_, Utxo>("SELECT * FROM gateway.utxo WHERE txid = $1")
//         .bind("b2")
//         .fetch_one(&repo.postgres_repo.pool)
//         .await;
//     assert_eq!(big?.status, "locked");
//
//     let small = sqlx::query_as::<_, Utxo>("SELECT * FROM gateway.utxo WHERE txid = $1")
//         .bind("b1")
//         .fetch_one(&repo.postgres_repo.pool)
//         .await;
//     assert_eq!(small?.status, "locked");
//
//     Ok(())
// }
//
// #[tokio::test]
// async fn test_insufficient_funds_rollback() -> Result<(), DbError> {
//     let repo = make_repo().await;
//     sqlx::query("TRUNCATE gateway.utxo RESTART IDENTITY CASCADE")
//         .execute(&repo.postgres_repo.pool)
//         .await?;
//
//     repo.insert_utxo(Utxo {
//         id: 0,
//         txid: "c1".into(),
//         vout: 0,
//         amount: 5,
//         rune_id: "rune4".into(),
//         sats_amount: None,
//         owner_pubkey: "p4".into(),
//         status: "unspent".into(),
//         block_height: Some(30),
//         created_at: Utc::now().naive_utc(),
//         updated_at: Utc::now().naive_utc(),
//     })
//     .await?;
//
//     repo.insert_utxo(Utxo {
//         id: 0,
//         txid: "c2".into(),
//         vout: 0,
//         amount: 10,
//         rune_id: "rune4".into(),
//         sats_amount: None,
//         owner_pubkey: "p4".into(),
//         status: "unspent".into(),
//         block_height: Some(31),
//         created_at: Utc::now().naive_utc(),
//         updated_at: Utc::now().naive_utc(),
//     })
//     .await?;
//
//     let res = repo.select_and_lock_utxos("rune4", 100).await;
//     assert!(res.is_err());
//
//     let unspent = repo.list_unspent("rune4").await?;
//     assert_eq!(unspent.len(), 2);
//     assert!(unspent.iter().all(|u| u.status == "unspent"));
//
//     Ok(())
// }
//
// #[tokio::test]
// async fn test_unlock_mark_spent_and_update_status() -> Result<(), DbError> {
//     let repo = make_repo().await;
//     sqlx::query("TRUNCATE gateway.utxo RESTART IDENTITY CASCADE")
//         .execute(&repo.postgres_repo.pool)
//         .await?;
//
//     let inserted = repo
//         .insert_utxo(Utxo {
//             id: 0,
//             txid: "d1".into(),
//             vout: 0,
//             amount: 100,
//             rune_id: "rune5".into(),
//             sats_amount: None,
//             owner_pubkey: "p5".into(),
//             status: "unspent".into(),
//             block_height: Some(40),
//             created_at: Utc::now().naive_utc(),
//             updated_at: Utc::now().naive_utc(),
//         })
//         .await?;
//
//     let locked = repo.select_and_lock_utxos("rune5", 50).await?;
//     assert!(locked.iter().any(|u| u.id == inserted.id));
//
//     UtxoManager::unlock_utxos(&repo, &[inserted.id as i64]).await?;
//     let after_unlock = sqlx::query_as::<_, Utxo>("SELECT * FROM gateway.utxo WHERE id = $1")
//         .bind(inserted.id)
//         .fetch_one(&repo.postgres_repo.pool)
//         .await;
//     assert_eq!(after_unlock?.status, "unspent");
//
//     repo.update_status(&inserted.txid, inserted.vout, "archived").await?;
//     let after_update = sqlx::query_as::<_, Utxo>("SELECT * FROM gateway.utxo WHERE id = $1")
//         .bind(inserted.id)
//         .fetch_one(&repo.postgres_repo.pool)
//         .await;
//     assert_eq!(after_update?.status, "archived");
//
//     repo.update_status(&inserted.txid, inserted.vout, "locked").await?;
//     repo.mark_spent(&[inserted.id as i64]).await?;
//     let after_spent = sqlx::query_as::<_, Utxo>("SELECT * FROM gateway.utxo WHERE id = $1")
//         .bind(inserted.id)
//         .fetch_one(&repo.postgres_repo.pool)
//         .await;
//     assert_eq!(after_spent?.status, "spent");
//
//     Ok(())
// }
//
// #[tokio::test]
// async fn test_concurrent_selection_one_wins() -> Result<(), DbError> {
//     let repo = make_repo().await;
//     sqlx::query("TRUNCATE gateway.utxo RESTART IDENTITY CASCADE")
//         .execute(&repo.postgres_repo.pool)
//         .await?;
//
//     repo.insert_utxo(Utxo {
//         id: 0,
//         txid: "e1".into(),
//         vout: 0,
//         amount: 50,
//         rune_id: "rune_conc".into(),
//         sats_amount: None,
//         owner_pubkey: "pc".into(),
//         status: "unspent".into(),
//         block_height: Some(50),
//         created_at: Utc::now().naive_utc(),
//         updated_at: Utc::now().naive_utc(),
//     })
//     .await?;
//     repo.insert_utxo(Utxo {
//         id: 0,
//         txid: "e2".into(),
//         vout: 0,
//         amount: 50,
//         rune_id: "rune_conc".into(),
//         sats_amount: None,
//         owner_pubkey: "pc".into(),
//         status: "unspent".into(),
//         block_height: Some(51),
//         created_at: Utc::now().naive_utc(),
//         updated_at: Utc::now().naive_utc(),
//     })
//     .await?;
//
//     let f1 = repo.select_and_lock_utxos("rune_conc", 100);
//     let f2 = repo.select_and_lock_utxos("rune_conc", 100);
//     let (r1, r2) = tokio::join!(f1, f2);
//
//     let ok_count = [r1.as_ref().ok(), r2.as_ref().ok()]
//         .iter()
//         .filter(|x| x.is_some())
//         .count();
//     assert!(ok_count >= 1, "at least one success");
//     assert!(ok_count <= 1, "once more - at least one success");
//
//     let remaining_unspent = repo.list_unspent("rune_conc").await?;
//
//     assert!(remaining_unspent.len() == 0 || remaining_unspent.len() == 2 || remaining_unspent.len() == 1);
//
//     Ok(())
// }
//
// fn create_test_utxo(
//     txid: &str,
//     vout: i32,
//     amount: i64,
//     sats_amount: Option<i64>,
//     rune_id: &str,
//     status: &str,
//     block_height: Option<i64>,
// ) -> Utxo {
//     Utxo {
//         id: 0,
//         txid: txid.to_string(),
//         vout,
//         amount,
//         sats_amount,
//         rune_id: rune_id.to_string(),
//         owner_pubkey: "test_pubkey".to_string(),
//         status: status.to_string(),
//         block_height,
//         created_at: Utc::now().naive_utc(),
//         updated_at: Utc::now().naive_utc(),
//     }
// }
//
// fn create_test_utxos() -> Vec<Utxo> {
//     vec![
//         create_test_utxo("test_txid_1", 0, 1000, Some(546), "test_rune_1", "unspent", None),
//         create_test_utxo("test_txid_2", 1, 2000, Some(1000), "test_rune_2", "unspent", None),
//         create_test_utxo("test_txid_3", 2, 3000, None, "test_rune_3", "pending", None),
//     ]
// }
//
// async fn cleanup_test_db(repo: &LocalDbStorage) {
//     sqlx::query("TRUNCATE gateway.utxo RESTART IDENTITY CASCADE")
//         .execute(&repo.postgres_repo.pool)
//         .await
//         .unwrap();
// }
//
// #[tokio::test]
// async fn test_insert_utxo() -> Result<(), DbError> {
//     let repo = make_repo().await;
//     cleanup_test_db(&repo).await;
//
//     let test_utxo = create_test_utxo("test_tx_1", 0, 1000, Some(546), "test_rune_id", "unspent", Some(100));
//
//     let inserted = repo.insert_utxo(test_utxo.clone()).await?;
//
//     assert_eq!(inserted.txid, test_utxo.txid);
//     assert_eq!(inserted.amount, test_utxo.amount);
//     assert_eq!(inserted.sats_amount, test_utxo.sats_amount);
//     assert_eq!(inserted.status, "unspent");
//     assert!(inserted.id > 0);
//
//     Ok(())
// }
//
// #[tokio::test]
// async fn test_insert_pending_utxo() -> Result<(), DbError> {
//     let repo = make_repo().await;
//     cleanup_test_db(&repo).await;
//
//     // let test_utxo = create_test_utxo(
//     //     "test_tx_pending",
//     //     1,
//     //     2000,
//     //     Some(1000),
//     //     "test_rune_id",
//     //     "unspent",
//     //     None,
//     // );
//
//     let test_utxos = create_test_utxos();
//
//     let inserted = repo.insert_pending_utxo(test_utxos.clone()).await?;
//
//     assert_eq!(inserted.status, "pending");
//     assert_eq!(inserted.block_height, None);
//
//     Ok(())
// }
//
// #[tokio::test]
// async fn test_update_status() -> Result<(), DbError> {
//     let repo = make_repo().await;
//     cleanup_test_db(&repo).await;
//
//     let test_utxo = create_test_utxo(
//         "test_tx_status",
//         0,
//         1500,
//         Some(600),
//         "test_rune_id",
//         "unspent",
//         Some(200),
//     );
//     repo.insert_utxo(test_utxo).await?;
//
//     repo.update_status("test_tx_status", 0, "spent").await?;
//
//     let updated_utxo = sqlx::query_as::<_, Utxo>("SELECT * FROM gateway.utxo WHERE txid = $1 AND vout = $2")
//         .bind("test_tx_status")
//         .bind(0)
//         .fetch_one(&repo.postgres_repo.pool)
//         .await?;
//
//     assert_eq!(updated_utxo.status, "spent");
//
//     let utxos = repo.list_unspent("test_rune_id").await?;
//     assert_eq!(utxos.len(), 0);
//
//     Ok(())
// }
//
// #[tokio::test]
// async fn test_update_status_not_found() -> Result<(), DbError> {
//     let repo = make_repo().await;
//     cleanup_test_db(&repo).await;
//
//     let result = repo.update_status("nonexistent_tx", 0, "spent").await;
//     assert!(result.is_err());
//     assert!(matches!(result.unwrap_err(), DbError::NotFound(_)));
//
//     Ok(())
// }
//
// #[tokio::test]
// async fn test_list_unspent_includes_pending() -> Result<(), DbError> {
//     let repo = make_repo().await;
//     cleanup_test_db(&repo).await;
//
//     let unspent_utxo = create_test_utxo("unspent_tx", 0, 1000, Some(546), "test_rune_id", "unspent", Some(100));
//     repo.insert_utxo(unspent_utxo).await?;
//
//     // let pending_utxo = create_test_utxo(
//     //     "pending_tx",
//     //     0,
//     //     2000,
//     //     Some(1000),
//     //     "test_rune_id",
//     //     "pending",
//     //     None,
//     // );
//
//     let test_utxos = create_test_utxos();
//     repo.insert_pending_utxo(test_utxos).await?;
//
//     let spent_utxo = create_test_utxo("spent_tx", 0, 500, Some(300), "test_rune_id", "spent", Some(99));
//     repo.insert_utxo(spent_utxo).await?;
//
//     let utxos = repo.list_unspent("test_rune_id").await?;
//
//     assert_eq!(utxos.len(), 2);
//
//     let statuses: Vec<&str> = utxos.iter().map(|u| u.status.as_str()).collect();
//     assert!(statuses.contains(&"unspent"));
//     assert!(statuses.contains(&"pending"));
//
//     Ok(())
// }
//
// #[tokio::test]
// async fn test_select_and_lock_utxos() -> Result<(), DbError> {
//     let repo = make_repo().await;
//     cleanup_test_db(&repo).await;
//
//     let utxos = vec![
//         create_test_utxo("tx1", 0, 500, Some(546), "test_rune", "unspent", Some(100)),
//         create_test_utxo("tx2", 0, 1000, Some(546), "test_rune", "unspent", Some(101)),
//         create_test_utxo("tx3", 0, 1500, Some(546), "test_rune", "unspent", Some(102)),
//     ];
//
//     for utxo in utxos {
//         repo.insert_utxo(utxo).await?;
//     }
//
//     let locked = repo.select_and_lock_utxos("test_rune", 1200).await?;
//
//     assert_eq!(locked.len(), 2);
//     assert_eq!(locked[0].amount, 500);
//     assert_eq!(locked[1].amount, 1000);
//
//     for utxo in &locked {
//         assert_eq!(utxo.status, "locked");
//     }
//
//     let tx1_status = sqlx::query_as::<_, Utxo>("SELECT * FROM gateway.utxo WHERE txid = $1")
//         .bind("tx1")
//         .fetch_one(&repo.postgres_repo.pool)
//         .await?;
//     assert_eq!(tx1_status.status, "locked");
//
//     let tx2_status = sqlx::query_as::<_, Utxo>("SELECT * FROM gateway.utxo WHERE txid = $1")
//         .bind("tx2")
//         .fetch_one(&repo.postgres_repo.pool)
//         .await?;
//     assert_eq!(tx2_status.status, "locked");
//
//     let tx3_status = sqlx::query_as::<_, Utxo>("SELECT * FROM gateway.utxo WHERE txid = $1")
//         .bind("tx3")
//         .fetch_one(&repo.postgres_repo.pool)
//         .await?;
//     assert_eq!(tx3_status.status, "unspent");
//
//     Ok(())
// }
//
// #[tokio::test]
// async fn test_select_and_lock_insufficient_funds() -> Result<(), DbError> {
//     let repo = make_repo().await;
//     cleanup_test_db(&repo).await;
//
//     let utxo = create_test_utxo("tx1", 0, 500, Some(546), "test_rune", "unspent", Some(100));
//     repo.insert_utxo(utxo).await?;
//
//     let result = repo.select_and_lock_utxos("test_rune", 1000).await;
//
//     assert!(result.is_err());
//     assert!(matches!(result.unwrap_err(), DbError::BadRequest(_)));
//
//     let utxo_status = sqlx::query_as::<_, Utxo>("SELECT * FROM gateway.utxo WHERE txid = $1")
//         .bind("tx1")
//         .fetch_one(&repo.postgres_repo.pool)
//         .await?;
//     assert_eq!(utxo_status.status, "unspent");
//
//     Ok(())
// }
//
// #[tokio::test]
// async fn test_unlock_utxos() -> Result<(), DbError> {
//     let repo = make_repo().await;
//     cleanup_test_db(&repo).await;
//
//     let utxo = create_test_utxo("tx1", 0, 1000, Some(546), "test_rune", "unspent", Some(100));
//     let inserted = repo.insert_utxo(utxo).await?;
//
//     let locked = repo.select_and_lock_utxos("test_rune", 500).await?;
//     assert_eq!(locked.len(), 1);
//
//     let utxo_ids: Vec<i64> = locked.iter().map(|u| u.id as i64).collect();
//     UtxoStorage::unlock_utxos(&repo, &utxo_ids).await?;
//
//     let utxo_status = sqlx::query_as::<_, Utxo>("SELECT * FROM gateway.utxo WHERE id = $1")
//         .bind(inserted.id)
//         .fetch_one(&repo.postgres_repo.pool)
//         .await?;
//     assert_eq!(utxo_status.status, "unspent");
//
//     Ok(())
// }
//
// #[tokio::test]
// async fn test_unlock_empty_list() -> Result<(), DbError> {
//     let repo = make_repo().await;
//
//     let result = UtxoStorage::unlock_utxos(&repo, &[]).await;
//     assert!(result.is_ok());
//
//     Ok(())
// }
//
// #[tokio::test]
// async fn test_set_block_height() -> Result<(), DbError> {
//     let repo = make_repo().await;
//     cleanup_test_db(&repo).await;
//
//     //let utxo = create_test_utxo("test_tx_height", 0, 1000, Some(546), "test_rune", "pending", None);
//     let test_utxos = create_test_utxos();
//     repo.insert_pending_utxo(test_utxos).await?;
//
//     repo.set_block_height("test_tx_height".parse().unwrap(), 800000).await?;
//
//     let updated_utxo = sqlx::query_as::<_, Utxo>("SELECT * FROM gateway.utxo WHERE txid = $1")
//         .bind("test_tx_height")
//         .fetch_one(&repo.postgres_repo.pool)
//         .await?;
//     assert_eq!(updated_utxo.block_height, Some(800000));
//
//     Ok(())
// }
//
// #[tokio::test]
// async fn test_set_block_height_not_found() -> Result<(), DbError> {
//     let repo = make_repo().await;
//     cleanup_test_db(&repo).await;
//
//     let result = repo.set_block_height("nonexistent_tx".parse().unwrap(), 800000).await;
//     assert!(result.is_err());
//     assert!(matches!(result.unwrap_err(), DbError::NotFound(_)));
//
//     Ok(())
// }
//
// #[tokio::test]
// async fn test_concurrent_select_and_lock() -> Result<(), DbError> {
//     let repo = make_repo().await;
//     cleanup_test_db(&repo).await;
//
//     let utxo = create_test_utxo(
//         "concurrent_unique_tx",
//         0,
//         1000,
//         Some(546),
//         "concurrent_rune",
//         "unspent",
//         Some(100),
//     );
//     repo.insert_utxo(utxo).await?;
//
//     let f1 = repo.select_and_lock_utxos("concurrent_rune", 500);
//     let f2 = repo.select_and_lock_utxos("concurrent_rune", 500);
//
//     let (result1, result2) = tokio::join!(f1, f2);
//
//     let success_count = [result1.as_ref(), result2.as_ref()]
//         .iter()
//         .filter(|r| r.is_ok())
//         .count();
//
//     assert!(success_count >= 1, "At least one should succeed");
//     assert!(success_count <= 1, "At most one should succeed");
//
//     Ok(())
// }
//
// #[tokio::test]
// async fn test_comprehensive_flow() -> Result<(), DbError> {
//     let repo = make_repo().await;
//     cleanup_test_db(&repo).await;
//
//     let utxos = vec![
//         create_test_utxo("flow_tx1", 0, 100, Some(546), "flow_rune", "unspent", Some(100)),
//         create_test_utxo("flow_tx2", 0, 200, Some(546), "flow_rune", "unspent", Some(101)),
//         create_test_utxo("flow_tx3", 0, 300, Some(1000), "flow_rune", "unspent", Some(102)),
//     ];
//
//     for utxo in utxos {
//         repo.insert_utxo(utxo).await?;
//     }
//
//     let unspent = repo.list_unspent("flow_rune").await?;
//     assert_eq!(unspent.len(), 3);
//
//     let locked = repo.select_and_lock_utxos("flow_rune", 250).await?;
//     assert_eq!(locked.len(), 2);
//
//     //let change_utxo = create_test_utxo("change_tx", 0, 50, Some(546), "flow_rune", "pending", None);
//     let change_utxos = create_test_utxos();
//     repo.insert_pending_utxo(change_utxos).await?;
//
//     repo.update_status("flow_tx1", 0, "spent").await?;
//     repo.update_status("flow_tx2", 0, "spent").await?;
//
//     repo.set_block_height("change_tx".parse().unwrap(), 12345).await?;
//
//     let final_unspent = repo.list_unspent("flow_rune").await?;
//     assert_eq!(final_unspent.len(), 2);
//
//     let statuses: Vec<&str> = final_unspent.iter().map(|u| u.status.as_str()).collect();
//     assert!(statuses.contains(&"unspent"));
//     assert!(statuses.contains(&"pending"));
//
//     let change_with_height = final_unspent.iter().find(|u| u.txid == "change_tx").unwrap();
//     assert_eq!(change_with_height.block_height, Some(12345));
//
//     Ok(())
// }
//
// async fn setup_test_table(repo: &LocalDbStorage) {
//     sqlx::query(
//         r#"
//             CREATE TABLE IF NOT EXISTS gateway.session_requests (
//                 session_id UUID PRIMARY KEY,
//                 request_type TEXT NOT NULL,
//                 status TEXT NOT NULL DEFAULT 'pending',
//                 request JSONB NOT NULL,
//                 response JSONB,
//                 error TEXT,
//                 created_at TIMESTAMP DEFAULT now(),
//                 updated_at TIMESTAMP DEFAULT now()
//             );
//
//             CREATE INDEX IF NOT EXISTS idx_session_requests_status ON gateway.session_requests(status);
//             CREATE INDEX IF NOT EXISTS idx_session_requests_type ON gateway.session_requests(request_type);
//             "#,
//     )
//     .execute(&repo.postgres_repo.pool)
//     .await
//     .expect("Failed to create test table");
// }
//
// async fn cleanup_sessions(repo: &LocalDbStorage) {
//     sqlx::query("TRUNCATE gateway.session_requests")
//         .execute(&repo.postgres_repo.pool)
//         .await
//         .unwrap();
// }
//
// #[tokio::test]
// async fn test_create_session() -> Result<(), DbError> {
//     let repo = make_repo().await;
//     //setup_test_table(&repo).await;
//     cleanup_sessions(&repo).await;
//
//     let request_data = json!({
//         "rune_id": "test_rune_123",
//         "amount": 1000,
//         "recipient": "test_address"
//     });
//
//     let session_id = repo
//         .postgres_repo
//         .create_session(RequestType::SendRunes, request_data.clone())
//         .await?;
//
//     assert_eq!(session_id.to_string().len(), 36);
//
//     let session = repo.postgres_repo.get_session(session_id).await?;
//     assert_eq!(session.session_id, session_id);
//     assert_eq!(session.request_type, "send_runes");
//     assert_eq!(session.status, "pending");
//     assert_eq!(session.request, request_data);
//     assert!(session.response.is_none());
//     assert!(session.error.is_none());
//
//     Ok(())
// }
//
// #[tokio::test]
// async fn test_session_status_updates() -> Result<(), DbError> {
//     let repo = make_repo().await;
//     //setup_test_table(&repo).await;
//     cleanup_sessions(&repo).await;
//
//     let request_data = json!({"test": "data"});
//     let session_id = repo
//         .postgres_repo
//         .create_session(RequestType::CreateTransaction, request_data)
//         .await?;
//
//     repo.postgres_repo
//         .update_session_status(session_id, SessionStatus::InProgress)
//         .await?;
//
//     let session = repo.postgres_repo.get_session(session_id).await?;
//     assert_eq!(session.status, "in_progress");
//
//     repo.postgres_repo
//         .set_session_error(session_id, "Test error occurred")
//         .await?;
//
//     let session = repo.postgres_repo.get_session(session_id).await?;
//     assert_eq!(session.status, "failed");
//     assert_eq!(session.error, Some("Test error occurred".to_string()));
//
//     Ok(())
// }
//
// #[tokio::test]
// async fn test_session_success() -> Result<(), DbError> {
//     let repo = make_repo().await;
//     //setup_test_table(&repo).await;
//     cleanup_sessions(&repo).await;
//
//     let request_data = json!({"action": "test"});
//     let session_id = repo
//         .postgres_repo
//         .create_session(RequestType::BroadcastTransaction, request_data)
//         .await?;
//
//     let response_data = json!({
//         "txid": "abc123def456",
//         "block_height": 800000
//     });
//
//     repo.postgres_repo
//         .set_session_success(session_id, response_data.clone())
//         .await?;
//
//     let session = repo.postgres_repo.get_session(session_id).await?;
//     assert_eq!(session.status, "success");
//     assert_eq!(session.response, Some(response_data));
//     assert!(session.error.is_none());
//
//     Ok(())
// }
//
// #[tokio::test]
// async fn test_list_sessions() -> Result<(), DbError> {
//     let repo = make_repo().await;
//     //setup_test_table(&repo).await;
//     cleanup_sessions(&repo).await;
//
//     let session1 = repo
//         .postgres_repo
//         .create_session(RequestType::SendRunes, json!({"test": "1"}))
//         .await?;
//
//     let session2 = repo
//         .postgres_repo
//         .create_session(RequestType::CreateTransaction, json!({"test": "2"}))
//         .await?;
//
//     repo.postgres_repo
//         .update_session_status(session1, SessionStatus::Success)
//         .await?;
//     repo.postgres_repo
//         .update_session_status(session2, SessionStatus::Failed)
//         .await?;
//
//     let all_sessions = repo.postgres_repo.list_sessions(Some(10), None).await?;
//     assert_eq!(all_sessions.len(), 2);
//
//     let success_sessions = repo
//         .postgres_repo
//         .list_sessions_by_status(SessionStatus::Success, Some(10))
//         .await?;
//     assert_eq!(success_sessions.len(), 1);
//     assert_eq!(success_sessions[0].session_id, session1);
//
//     let failed_sessions = repo
//         .postgres_repo
//         .list_sessions_by_status(SessionStatus::Failed, Some(10))
//         .await?;
//     assert_eq!(failed_sessions.len(), 1);
//     assert_eq!(failed_sessions[0].session_id, session2);
//
//     Ok(())
// }
//
// #[tokio::test]
// async fn test_session_tracker_helper() -> Result<(), DbError> {
//     let repo = make_repo().await;
//     //setup_test_table(&repo).await;
//     cleanup_sessions(&repo).await;
//
//     let tracker = SessionTracker::new(&repo);
//
//     let session_id = tracker
//         .start_session(
//             RequestType::GenerateFrostSignature,
//             json!({"message": "test_signature"}),
//         )
//         .await?;
//
//     let status = tracker.get_session_status(session_id).await?;
//     assert!(matches!(status, SessionStatus::InProgress));
//
//     let result_data = json!({"signature": "abcd1234"});
//     tracker.complete_session(session_id, result_data.clone()).await?;
//
//     let session = repo.postgres_repo.get_session(session_id).await?;
//     assert_eq!(session.status, "success");
//     assert_eq!(session.response, Some(result_data));
//
//     Ok(())
// }
//
// #[tokio::test]
// async fn test_session_not_found() -> Result<(), DbError> {
//     let repo = make_repo().await;
//     //setup_test_table(&repo).await;
//     cleanup_sessions(&repo).await;
//
//     let non_existent_id = Uuid::new_v4();
//
//     let result = repo.postgres_repo.get_session(non_existent_id).await;
//     assert!(result.is_err());
//     assert!(matches!(result.unwrap_err(), DbError::NotFound(_)));
//
//     Ok(())
// }
//
// #[tokio::test]
// async fn test_concurrent_session_operations() -> Result<(), DbError> {
//     let repo = make_repo().await;
//     //setup_test_table(&repo).await;
//     cleanup_sessions(&repo).await;
//
//     let handles: Vec<_> = (0..5)
//         .map(|i| {
//             let repo_clone = repo.postgres_repo.clone();
//             tokio::spawn(async move {
//                 repo_clone
//                     .create_session(RequestType::SendRunes, json!({"batch_id": i}))
//                     .await
//             })
//         })
//         .collect();
//
//     let results: Result<Vec<_>, _> = futures::future::try_join_all(handles).await;
//     let session_ids: Result<Vec<_>, _> = results.unwrap().into_iter().collect();
//     let session_ids = session_ids?;
//
//     assert_eq!(session_ids.len(), 5);
//
//     let mut unique_ids = std::collections::HashSet::new();
//     for id in &session_ids {
//         assert!(unique_ids.insert(*id), "Duplicate session ID found");
//     }
//
//     let all_sessions = repo.postgres_repo.list_sessions(Some(10), None).await?;
//     assert_eq!(all_sessions.len(), 5);
//
//     Ok(())
// }
//
// #[tokio::test]
// async fn test_request_type_enum_conversion() {
//     let types = vec![
//         RequestType::SendRunes,
//         RequestType::CreateTransaction,
//         RequestType::BroadcastTransaction,
//         RequestType::GenerateFrostSignature,
//         RequestType::Other("custom_operation".to_string()),
//     ];
//
//     for original_type in types {
//         let type_string = original_type.to_string();
//         let converted_back = RequestType::from(type_string.clone());
//
//         match (&original_type, &converted_back) {
//             (RequestType::SendRunes, RequestType::SendRunes) => {}
//             (RequestType::CreateTransaction, RequestType::CreateTransaction) => {}
//             (RequestType::BroadcastTransaction, RequestType::BroadcastTransaction) => {}
//             (RequestType::GenerateFrostSignature, RequestType::GenerateFrostSignature) => {}
//             (RequestType::Other(s1), RequestType::Other(s2)) => assert_eq!(s1, s2),
//             _ => panic!("Type conversion failed for {}", type_string),
//         }
//     }
// }
