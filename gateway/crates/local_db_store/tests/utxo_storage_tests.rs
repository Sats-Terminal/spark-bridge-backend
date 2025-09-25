mod tests {
    use chrono::Utc;
    use gateway_local_db_store::storage::{LocalDbStorage, make_repo_with_config};
    // use gateway_runes_utxo_manager::traits::{CoinSelector, Utxo, UtxoManager, UtxoStatus, UtxoStorage};
    // use gateway_runes_utxo_manager::utxo_manager::GreedySelector;
    use gateway_local_db_store::schemas::utxo_storage::Utxo;
    use persistent_storage::error::DbError as DatabaseError;
    use persistent_storage::init::{PostgresPool, PostgresRepo};
    use sqlx::Executor;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering};

    pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn test_select_and_lock_basic_flow(db: PostgresPool) -> Result<(), DatabaseError> {
        let repo = make_repo_with_config(db).await;

        sqlx::query("TRUNCATE gateway.utxo RESTART IDENTITY CASCADE")
            .execute(&repo.postgres_repo.pool)
            .await
            .unwrap();

        let utxos = vec![
            Utxo {
                id: 0,
                txid: "t1".into(),
                vout: 0,
                amount: 10,
                rune_id: "rune1".into(),
                sats_amount: Some(0),
                owner_pubkey: "pub1".into(),
                status: UtxoStatus::Unspent,
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
                sats_amount: Some(0),
                owner_pubkey: "pub1".into(),
                status: UtxoStatus::Unspent,
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
                sats_amount: Some(0),
                owner_pubkey: "pub2".into(),
                status: UtxoStatus::Unspent,
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
        assert!(unspent.iter().all(|u| u.status == UtxoStatus::Unspent));

        let selector = GreedySelector { repo: repo.clone() };
        let selected = selector.select_utxos("rune1", 25).await?;

        assert_eq!(selected.len(), 2);
        assert_eq!(selected.iter().map(|u| u.amount).sum::<i64>(), 30);

        for u in selected {
            let db_u = sqlx::query_as::<_, Utxo>("SELECT * FROM gateway.utxo WHERE txid = $1 AND vout = $2")
                .bind(&u.txid)
                .bind(u.vout)
                .fetch_one(&repo.get_conn().await?)
                .await;
            assert_eq!(db_u?.status, UtxoStatus::Locked);
        }

        Ok(())
    }

    #[sqlx::test(migrator = "MIGRATOR")]

    async fn test_select_exact_and_single_large_utxo(db: PostgresPool) -> Result<(), DatabaseError> {
        let repo = make_repo_with_config(db).await;

        sqlx::query("TRUNCATE gateway.utxo RESTART IDENTITY CASCADE")
            .execute(&repo.postgres_repo.pool)
            .await?;

        repo.insert_utxo(Utxo {
            id: 0,
            txid: "a1".into(),
            vout: 0,
            amount: 15,
            rune_id: "rune2".into(),
            sats_amount: Some(0),
            owner_pubkey: "p2".into(),
            status: UtxoStatus::Unspent,
            block_height: Some(10),
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        })
        .await?;

        repo.insert_utxo(Utxo {
            id: 0,
            txid: "a2".into(),
            vout: 0,
            amount: 35,
            rune_id: "rune2".into(),
            sats_amount: Some(0),
            owner_pubkey: "p2".into(),
            status: UtxoStatus::Unspent,
            block_height: Some(11),
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        })
        .await?;

        let sel = repo.select_and_lock_utxos("rune2", 50).await?;
        assert_eq!(sel.len(), 2);
        assert_eq!(sel.iter().map(|u| u.amount).sum::<i64>(), 50);

        sqlx::query("TRUNCATE gateway.utxo RESTART IDENTITY CASCADE")
            .execute(&repo.postgres_repo.pool)
            .await?;

        repo.insert_utxo(Utxo {
            id: 0,
            txid: "b1".into(),
            vout: 0,
            amount: 5,
            rune_id: "rune3".into(),
            sats_amount: Some(0),
            owner_pubkey: "p3".into(),
            status: UtxoStatus::Unspent,
            block_height: Some(20),
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        })
        .await?;

        repo.insert_utxo(Utxo {
            id: 0,
            txid: "b2".into(),
            vout: 0,
            amount: 100,
            rune_id: "rune3".into(),
            sats_amount: Some(0),
            owner_pubkey: "p3".into(),
            status: UtxoStatus::Unspent,
            block_height: Some(21),
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        })
        .await?;

        let sel2 = repo.select_and_lock_utxos("rune3", 50).await?;
        assert_eq!(sel2.len(), 2);
        assert_eq!(sel2[0].amount, 5);
        assert_eq!(sel2[0].txid, "b1");

        let big = sqlx::query_as::<_, Utxo>("SELECT * FROM gateway.utxo WHERE txid = $1")
            .bind("b2")
            .fetch_one(&repo.postgres_repo.pool)
            .await;
        assert_eq!(big?.status, UtxoStatus::Locked);

        let small = sqlx::query_as::<_, Utxo>("SELECT * FROM gateway.utxo WHERE txid = $1")
            .bind("b1")
            .fetch_one(&repo.postgres_repo.pool)
            .await;
        assert_eq!(small?.status, UtxoStatus::Locked);

        Ok(())
    }

    #[sqlx::test(migrator = "MIGRATOR")]

    async fn test_insufficient_funds_rollback(db: PostgresPool) -> Result<(), DatabaseError> {
        let repo = make_repo_with_config(db).await;

        sqlx::query("TRUNCATE gateway.utxo RESTART IDENTITY CASCADE")
            .execute(&repo.postgres_repo.pool)
            .await?;

        repo.insert_utxo(Utxo {
            id: 0,
            txid: "c1".into(),
            vout: 0,
            amount: 5,
            rune_id: "rune4".into(),
            sats_amount: Some(0),
            owner_pubkey: "p4".into(),
            status: UtxoStatus::Unspent,
            block_height: Some(30),
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        })
        .await?;

        repo.insert_utxo(Utxo {
            id: 0,
            txid: "c2".into(),
            vout: 0,
            amount: 10,
            rune_id: "rune4".into(),
            sats_amount: Some(0),
            owner_pubkey: "p4".into(),
            status: UtxoStatus::Unspent,
            block_height: Some(31),
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        })
        .await?;

        let res = repo.select_and_lock_utxos("rune4", 100).await;
        assert!(res.is_err());

        let unspent = repo.list_unspent("rune4").await?;
        assert_eq!(unspent.len(), 2);
        assert!(unspent.iter().all(|u| u.status == UtxoStatus::Unspent));

        Ok(())
    }

    #[sqlx::test(migrator = "MIGRATOR")]

    async fn test_unlock_mark_spent_and_update_status(db: PostgresPool) -> Result<(), DatabaseError> {
        let repo = make_repo_with_config(db).await;

        sqlx::query("TRUNCATE gateway.utxo RESTART IDENTITY CASCADE")
            .execute(&repo.postgres_repo.pool)
            .await?;

        let inserted = repo
            .insert_utxo(Utxo {
                id: 0,
                txid: "d1".into(),
                vout: 0,
                amount: 100,
                rune_id: "rune5".into(),
                sats_amount: Some(0),
                owner_pubkey: "p5".into(),
                status: UtxoStatus::Unspent,
                block_height: Some(40),
                created_at: Utc::now().naive_utc(),
                updated_at: Utc::now().naive_utc(),
            })
            .await?;

        let locked = repo.select_and_lock_utxos("rune5", 50).await?;
        assert!(locked.iter().any(|u| u.id == inserted.id));

        repo.unlock_utxos(&[inserted.id as i64]).await?;
        let after_unlock = sqlx::query_as::<_, Utxo>("SELECT * FROM gateway.utxo WHERE id = $1")
            .bind(inserted.id)
            .fetch_one(&repo.postgres_repo.pool)
            .await;
        assert_eq!(after_unlock?.status, UtxoStatus::Unspent);

        repo.update_status(&inserted.txid, inserted.vout, UtxoStatus::Archived)
            .await?;
        let after_update = sqlx::query_as::<_, Utxo>("SELECT * FROM gateway.utxo WHERE id = $1")
            .bind(inserted.id)
            .fetch_one(&repo.postgres_repo.pool)
            .await;
        assert_eq!(after_update?.status, UtxoStatus::Archived);

        repo.update_status(&inserted.txid, inserted.vout, UtxoStatus::Locked)
            .await?;
        repo.mark_spent(&[inserted.id as i64]).await?;
        let after_spent = sqlx::query_as::<_, Utxo>("SELECT * FROM gateway.utxo WHERE id = $1")
            .bind(inserted.id)
            .fetch_one(&repo.postgres_repo.pool)
            .await;
        assert_eq!(after_spent?.status, UtxoStatus::Spent);

        Ok(())
    }

    #[sqlx::test(migrator = "MIGRATOR")]

    async fn test_concurrent_selection_one_wins(db: PostgresPool) -> Result<(), DatabaseError> {
        let repo = make_repo_with_config(db).await;

        sqlx::query("TRUNCATE gateway.utxo RESTART IDENTITY CASCADE")
            .execute(&repo.postgres_repo.pool)
            .await?;

        repo.insert_utxo(Utxo {
            id: 0,
            txid: "e1".into(),
            vout: 0,
            amount: 50,
            rune_id: "rune_conc".into(),
            sats_amount: Some(0),
            owner_pubkey: "pc".into(),
            status: UtxoStatus::Unspent,
            block_height: Some(50),
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        })
        .await?;
        repo.insert_utxo(Utxo {
            id: 0,
            txid: "e2".into(),
            vout: 0,
            amount: 50,
            rune_id: "rune_conc".into(),
            sats_amount: Some(0),
            owner_pubkey: "pc".into(),
            status: UtxoStatus::Unspent,
            block_height: Some(51),
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        })
        .await?;

        let f1 = repo.select_and_lock_utxos("rune_conc", 100);
        let f2 = repo.select_and_lock_utxos("rune_conc", 100);
        let (r1, r2) = tokio::join!(f1, f2);

        let ok_count = [r1.as_ref().ok(), r2.as_ref().ok()]
            .iter()
            .filter(|x| x.is_some())
            .count();
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
        status: UtxoStatus,
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
            status,
            block_height,
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        }
    }

    fn create_test_utxos() -> Vec<Utxo> {
        vec![
            create_test_utxo(
                "test_txid_1",
                0,
                1000,
                Some(546),
                "test_rune_1",
                UtxoStatus::Unspent,
                Some(0),
            ),
            create_test_utxo(
                "test_txid_2",
                1,
                2000,
                Some(1000),
                "test_rune_2",
                UtxoStatus::Unspent,
                Some(0),
            ),
            create_test_utxo(
                "test_txid_3",
                2,
                3000,
                Some(0),
                "test_rune_3",
                UtxoStatus::Pending,
                Some(0),
            ),
        ]
    }

    async fn cleanup_test_db(repo: Arc<LocalDbStorage>) {
        sqlx::query("TRUNCATE gateway.utxo, gateway.session_requests RESTART IDENTITY CASCADE")
            .execute(&repo.get_conn().await.unwrap())
            .await
            .unwrap();
    }

    #[sqlx::test(migrator = "MIGRATOR")]

    async fn test_insert_utxo(db: PostgresPool) -> Result<(), DatabaseError> {
        let repo = make_repo_with_config(db).await;

        cleanup_test_db(repo.clone()).await;

        let test_utxo = create_test_utxo(
            "test_tx_1",
            0,
            1000,
            Some(546),
            "test_rune_id",
            UtxoStatus::Unspent,
            Some(100),
        );

        let inserted = repo.insert_utxo(test_utxo.clone()).await?;

        assert_eq!(inserted.txid, test_utxo.txid);
        assert_eq!(inserted.amount, test_utxo.amount);
        assert_eq!(inserted.sats_amount, test_utxo.sats_amount);
        assert_eq!(inserted.status, UtxoStatus::Unspent);
        assert!(inserted.id > 0);

        Ok(())
    }

    #[sqlx::test(migrator = "MIGRATOR")]

    async fn test_insert_pending_utxo(db: PostgresPool) -> Result<(), DatabaseError> {
        let repo = make_repo_with_config(db).await;

        cleanup_test_db(repo.clone()).await;

        // let test_utxo = create_test_utxo(
        //     "test_tx_pending",
        //     1,
        //     2000,
        //     Some(1000),
        //     "test_rune_id",
        //     UtxoStatus::Unspent,
        //     None,
        // );

        let test_utxos = create_test_utxos();

        let inserted = repo.insert_pending_utxo(test_utxos.clone()).await?;

        assert_eq!(inserted.status, UtxoStatus::Pending);
        assert_eq!(inserted.block_height, None);

        Ok(())
    }

    #[sqlx::test(migrator = "MIGRATOR")]

    async fn test_update_status(db: PostgresPool) -> Result<(), DatabaseError> {
        let repo = make_repo_with_config(db).await;

        cleanup_test_db(repo.clone()).await;

        let test_utxo = create_test_utxo(
            "test_tx_status",
            0,
            1500,
            Some(600),
            "test_rune_id",
            UtxoStatus::Unspent,
            Some(200),
        );
        repo.insert_utxo(test_utxo).await?;

        repo.update_status("test_tx_status", 0, UtxoStatus::Spent).await?;

        let updated_utxo = sqlx::query_as::<_, Utxo>("SELECT * FROM gateway.utxo WHERE txid = $1 AND vout = $2")
            .bind("test_tx_status")
            .bind(0)
            .fetch_one(&repo.postgres_repo.pool)
            .await?;

        assert_eq!(updated_utxo.status, UtxoStatus::Spent);

        let utxos = repo.list_unspent("test_rune_id").await?;
        assert_eq!(utxos.len(), 0);

        Ok(())
    }

    #[sqlx::test(migrator = "MIGRATOR")]

    async fn test_update_status_not_found(db: PostgresPool) -> Result<(), DatabaseError> {
        let repo = make_repo_with_config(db).await;

        cleanup_test_db(repo.clone()).await;

        let result = repo.update_status("nonexistent_tx", 0, UtxoStatus::Spent).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), DatabaseError::NotFound(_)));

        Ok(())
    }

    #[sqlx::test(migrator = "MIGRATOR")]

    async fn test_list_unspent_includes_pending(db: PostgresPool) -> Result<(), DatabaseError> {
        let repo = make_repo_with_config(db).await;

        cleanup_test_db(repo.clone()).await;

        let unspent_utxo = create_test_utxo(
            "unspent_tx",
            0,
            1000,
            Some(546),
            "test_rune_id",
            UtxoStatus::Unspent,
            Some(100),
        );
        repo.insert_utxo(unspent_utxo).await?;

        let test_utxos = create_test_utxos();
        repo.insert_pending_utxo(test_utxos).await?;

        let spent_utxo = create_test_utxo(
            "spent_tx",
            0,
            500,
            Some(300),
            "test_rune_id",
            UtxoStatus::Spent,
            Some(99),
        );
        repo.insert_utxo(spent_utxo).await?;

        let utxos = repo.list_unspent("test_rune_id").await?;

        assert_eq!(utxos.len(), 1);

        let statuses: Vec<UtxoStatus> = utxos.iter().map(|u| u.status).collect();
        assert!(statuses.contains(&UtxoStatus::Unspent));

        Ok(())
    }

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn test_select_and_lock_utxos(db: PostgresPool) -> Result<(), DatabaseError> {
        let repo = make_repo_with_config(db).await;

        cleanup_test_db(repo.clone()).await;

        let utxos = vec![
            create_test_utxo("tx1", 0, 500, Some(546), "test_rune", UtxoStatus::Unspent, Some(100)),
            create_test_utxo("tx2", 0, 1000, Some(546), "test_rune", UtxoStatus::Unspent, Some(101)),
            create_test_utxo("tx3", 0, 1500, Some(546), "test_rune", UtxoStatus::Unspent, Some(102)),
        ];

        for utxo in utxos {
            repo.insert_utxo(utxo).await?;
        }

        let locked = repo.select_and_lock_utxos("test_rune", 1200).await?;

        assert_eq!(locked.len(), 2);
        assert_eq!(locked[0].amount, 500);
        assert_eq!(locked[1].amount, 1000);

        for utxo in &locked {
            assert_eq!(utxo.status, UtxoStatus::Locked);
        }

        let tx1_status = sqlx::query_as::<_, Utxo>("SELECT * FROM gateway.utxo WHERE txid = $1")
            .bind("tx1")
            .fetch_one(&repo.postgres_repo.pool)
            .await?;
        assert_eq!(tx1_status.status, UtxoStatus::Locked);

        let tx2_status = sqlx::query_as::<_, Utxo>("SELECT * FROM gateway.utxo WHERE txid = $1")
            .bind("tx2")
            .fetch_one(&repo.postgres_repo.pool)
            .await?;
        assert_eq!(tx2_status.status, UtxoStatus::Locked);

        let tx3_status = sqlx::query_as::<_, Utxo>("SELECT * FROM gateway.utxo WHERE txid = $1")
            .bind("tx3")
            .fetch_one(&repo.postgres_repo.pool)
            .await?;
        assert_eq!(tx3_status.status, UtxoStatus::Unspent);

        Ok(())
    }

    #[sqlx::test(migrator = "MIGRATOR")]

    async fn test_select_and_lock_insufficient_funds(db: PostgresPool) -> Result<(), DatabaseError> {
        let repo = make_repo_with_config(db).await;

        cleanup_test_db(repo.clone()).await;

        let utxo = create_test_utxo("tx1", 0, 500, Some(546), "test_rune", UtxoStatus::Unspent, Some(100));
        repo.insert_utxo(utxo).await?;

        let result = repo.select_and_lock_utxos("test_rune", 1000).await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), DatabaseError::BadRequest(_)));

        let utxo_status = sqlx::query_as::<_, Utxo>("SELECT * FROM gateway.utxo WHERE txid = $1")
            .bind("tx1")
            .fetch_one(&repo.postgres_repo.pool)
            .await?;
        assert_eq!(utxo_status.status, UtxoStatus::Unspent);

        Ok(())
    }

    #[sqlx::test(migrator = "MIGRATOR")]

    async fn test_unlock_utxos(db: PostgresPool) -> Result<(), DatabaseError> {
        let repo = make_repo_with_config(db).await;

        cleanup_test_db(repo.clone()).await;

        let utxo = create_test_utxo("tx1", 0, 1000, Some(546), "test_rune", UtxoStatus::Unspent, Some(100));
        let inserted = repo.insert_utxo(utxo).await?;

        let locked = repo.select_and_lock_utxos("test_rune", 500).await?;
        assert_eq!(locked.len(), 1);

        let utxo_ids: Vec<i64> = locked.iter().map(|u| u.id as i64).collect();
        repo.unlock_utxos_ids(&utxo_ids).await?;

        let utxo_status = sqlx::query_as::<_, Utxo>("SELECT * FROM gateway.utxo WHERE id = $1")
            .bind(inserted.id)
            .fetch_one(&repo.postgres_repo.pool)
            .await?;
        assert_eq!(utxo_status.status, UtxoStatus::Unspent);

        Ok(())
    }

    #[sqlx::test(migrator = "MIGRATOR")]

    async fn test_unlock_empty_list(db: PostgresPool) -> Result<(), DatabaseError> {
        let repo = make_repo_with_config(db).await;

        let result = repo.unlock_utxos_ids(&[]).await;
        assert!(result.is_ok());

        Ok(())
    }

    #[sqlx::test(migrator = "MIGRATOR")]

    async fn test_set_block_height_not_found(db: PostgresPool) -> Result<(), DatabaseError> {
        let repo = make_repo_with_config(db).await;

        cleanup_test_db(repo.clone()).await;

        let result = repo.set_block_height("a".repeat(64).parse().unwrap(), 800000).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), DatabaseError::NotFound(_)));

        Ok(())
    }

    #[sqlx::test(migrator = "MIGRATOR")]

    async fn test_concurrent_select_and_lock(db: PostgresPool) -> Result<(), DatabaseError> {
        let repo = make_repo_with_config(db).await;

        cleanup_test_db(repo.clone()).await;

        let utxo = create_test_utxo(
            "concurrent_unique_tx",
            0,
            1000,
            Some(546),
            "concurrent_rune",
            UtxoStatus::Unspent,
            Some(100),
        );
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

    static COUNTER: AtomicU64 = AtomicU64::new(1);

    fn generate_unique_txid(prefix: &str) -> String {
        let counter = COUNTER.fetch_add(1, Ordering::SeqCst);
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        format!("{:016x}{:016x}{:032x}", counter, timestamp, prefix.len())
    }

    fn create_unique_test_utxo(
        prefix: &str,
        vout: i32,
        amount: i64,
        sats_amount: Option<i64>,
        rune_id: &str,
        status: UtxoStatus,
        block_height: Option<i64>,
    ) -> Utxo {
        Utxo {
            id: 0,
            txid: generate_unique_txid(prefix),
            vout,
            amount,
            sats_amount,
            rune_id: rune_id.to_string(),
            owner_pubkey: "test_pubkey".to_string(),
            status: status,
            block_height,
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        }
    }

    fn create_unique_test_utxos(base_name: &str) -> Vec<Utxo> {
        vec![
            create_unique_test_utxo(
                &format!("{}_1", base_name),
                0,
                1000,
                Some(546),
                "test_rune_1",
                UtxoStatus::Unspent,
                None,
            ),
            create_unique_test_utxo(
                &format!("{}_2", base_name),
                1,
                2000,
                Some(1000),
                "test_rune_2",
                UtxoStatus::Unspent,
                None,
            ),
            create_unique_test_utxo(
                &format!("{}_3", base_name),
                2,
                3000,
                None,
                "test_rune_3",
                UtxoStatus::Pending,
                None,
            ),
        ]
    }

    #[sqlx::test(migrator = "MIGRATOR")]

    async fn test_comprehensive_flow(db: PostgresPool) -> Result<(), DatabaseError> {
        let repo = make_repo_with_config(db).await;

        cleanup_test_db(repo.clone()).await;

        let utxos = vec![
            create_unique_test_utxo(
                "flow_tx1",
                0,
                100,
                Some(546),
                "flow_rune",
                UtxoStatus::Unspent,
                Some(100),
            ),
            create_unique_test_utxo(
                "flow_tx2",
                0,
                200,
                Some(546),
                "flow_rune",
                UtxoStatus::Unspent,
                Some(101),
            ),
            create_unique_test_utxo(
                "flow_tx3",
                0,
                300,
                Some(1000),
                "flow_rune",
                UtxoStatus::Unspent,
                Some(102),
            ),
        ];

        let flow_tx1_id = utxos[0].txid.clone();
        let flow_tx2_id = utxos[1].txid.clone();

        for utxo in utxos {
            repo.insert_utxo(utxo).await?;
        }

        let unspent = repo.list_unspent("flow_rune").await?;
        assert_eq!(unspent.len(), 3);

        let locked = repo.select_and_lock_utxos("flow_rune", 250).await?;
        assert_eq!(locked.len(), 2);

        let first_change_utxo =
            create_unique_test_utxo("first_change", 0, 50, Some(546), "flow_rune", UtxoStatus::Pending, None);
        repo.insert_pending_utxo(vec![first_change_utxo]).await?;

        repo.update_status(&flow_tx1_id, 0, UtxoStatus::Spent).await?;
        repo.update_status(&flow_tx2_id, 0, UtxoStatus::Spent).await?;

        let second_change_utxo = create_unique_test_utxo(
            "second_change",
            0,
            25,
            Some(546),
            "flow_rune",
            UtxoStatus::Pending,
            None,
        );
        let change_txid = second_change_utxo.txid.clone();

        repo.insert_pending_utxo(vec![second_change_utxo]).await?;
        repo.set_block_height(change_txid.parse().unwrap(), 12345).await?;

        let final_unspent = repo.list_unspent("flow_rune").await?;
        assert_eq!(final_unspent.len(), 3);

        let statuses: Vec<UtxoStatus> = final_unspent.iter().map(|u| u.status).collect();
        assert!(statuses.contains(&UtxoStatus::Unspent));
        assert!(statuses.contains(&UtxoStatus::Pending));

        let change_with_height = final_unspent.iter().find(|u| u.txid == change_txid).unwrap();
        assert_eq!(change_with_height.block_height, Some(12345));

        Ok(())
    }

    async fn cleanup_sessions(repo: Arc<LocalDbStorage>) {
        sqlx::query("TRUNCATE gateway.session_requests")
            .execute(&repo.get_conn().await.unwrap())
            .await
            .unwrap();
    }

    #[sqlx::test(migrator = "MIGRATOR")]

    async fn test_comprehensive_flow_v2(db: PostgresPool) -> Result<(), DatabaseError> {
        let repo = make_repo_with_config(db).await;

        cleanup_test_db(repo.clone()).await;

        let initial_utxos = vec![
            create_unique_test_utxo(
                "initial_1",
                0,
                100,
                Some(546),
                "test_rune",
                UtxoStatus::Unspent,
                Some(100),
            ),
            create_unique_test_utxo(
                "initial_2",
                0,
                200,
                Some(546),
                "test_rune",
                UtxoStatus::Unspent,
                Some(101),
            ),
            create_unique_test_utxo(
                "initial_3",
                0,
                300,
                Some(1000),
                "test_rune",
                UtxoStatus::Unspent,
                Some(102),
            ),
        ];

        let mut inserted_utxos = Vec::new();
        for utxo in initial_utxos {
            let inserted = repo.insert_utxo(utxo).await?;
            inserted_utxos.push(inserted);
        }

        let unspent = repo.list_unspent("test_rune").await?;
        assert_eq!(unspent.len(), 3);

        let locked = repo.select_and_lock_utxos("test_rune", 250).await?;
        assert_eq!(locked.len(), 2);

        let change_utxo = create_unique_test_utxo(
            "change_output",
            0,
            50,
            Some(546),
            "test_rune",
            UtxoStatus::Pending,
            None,
        );
        let change_txid = change_utxo.txid.clone();
        repo.insert_pending_utxo(vec![change_utxo]).await?;

        for locked_utxo in &locked {
            repo.update_status(&locked_utxo.txid, locked_utxo.vout, UtxoStatus::Spent)
                .await?;
        }

        repo.set_block_height(change_txid.parse().unwrap(), 12345).await?;

        let final_unspent = repo.list_unspent("test_rune").await?;
        assert_eq!(final_unspent.len(), 2);

        let statuses: Vec<UtxoStatus> = final_unspent.iter().map(|u| u.status).collect();
        assert!(statuses.contains(&UtxoStatus::Unspent));
        assert!(statuses.contains(&UtxoStatus::Pending));

        let change_with_height = final_unspent.iter().find(|u| u.txid == change_txid).unwrap();
        assert_eq!(change_with_height.block_height, Some(12345));

        Ok(())
    }
}
