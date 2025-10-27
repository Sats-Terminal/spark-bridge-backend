mod utils;
mod tests {
    use bitcoin::{Network, OutPoint, Txid};
    use gateway_local_db_store::schemas::utxo_storage::{Utxo, UtxoStatus, UtxoStorage};
    use gateway_local_db_store::storage::LocalDbStorage;
    use persistent_storage::error::DbError;
    use persistent_storage::init::PostgresRepo;
    use std::str::FromStr;
    use std::sync::Arc;

    pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

    fn make_test_repo(db: sqlx::PgPool) -> Arc<LocalDbStorage> {
        Arc::new(LocalDbStorage {
            postgres_repo: PostgresRepo { pool: db },
            network: Network::Regtest,
        })
    }

    async fn cleanup_test_db(repo: &LocalDbStorage) {
        sqlx::query("TRUNCATE gateway.utxo RESTART IDENTITY CASCADE")
            .execute(&repo.postgres_repo.pool)
            .await
            .unwrap();
    }

    const SATS_AMOUNT: u64 = 546;
    const RUNE_AMOUNT: u64 = 1000;

    fn create_test_utxo(txid: &str, vout: u32, rune_amount: u64, rune_id: &str, status: UtxoStatus) -> Utxo {
        Utxo {
            outpoint: OutPoint {
                txid: Txid::from_str(txid).unwrap(),
                vout,
            },
            btc_address: bitcoin::Address::from_str("bcrt1qw508d6qejxtdg4y5r3zarvary0c5xw7kygt080")
                .unwrap()
                .require_network(Network::Regtest)
                .unwrap(),
            rune_amount,
            rune_id: rune_id.to_string(),
            status,
            sats_amount: SATS_AMOUNT,
        }
    }

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn test_insert_utxo(db: sqlx::PgPool) -> Result<(), DbError> {
        let repo = make_test_repo(db);
        cleanup_test_db(&repo).await;

        let utxo = create_test_utxo(
            "0000000000000000000000000000000000000000000000000000000000000001",
            0,
            RUNE_AMOUNT,
            "test_rune_1",
            UtxoStatus::Confirmed,
        );

        let inserted = repo.insert_utxo(utxo.clone()).await?;

        assert_eq!(inserted.outpoint, utxo.outpoint);
        assert_eq!(inserted.rune_amount, utxo.rune_amount);
        assert_eq!(inserted.rune_id, utxo.rune_id);
        assert_eq!(inserted.status, UtxoStatus::Confirmed);

        Ok(())
    }

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn test_insert_utxo_conflict_update(db: sqlx::PgPool) -> Result<(), DbError> {
        let repo = make_test_repo(db);
        cleanup_test_db(&repo).await;

        let utxo1 = create_test_utxo(
            "0000000000000000000000000000000000000000000000000000000000000002",
            0,
            RUNE_AMOUNT,
            "rune_1",
            UtxoStatus::Pending,
        );
        repo.insert_utxo(utxo1).await?;

        let utxo2 = create_test_utxo(
            "0000000000000000000000000000000000000000000000000000000000000002",
            0,
            RUNE_AMOUNT,
            "rune_2",
            UtxoStatus::Confirmed,
        );
        let updated = repo.insert_utxo(utxo2).await?;

        assert_eq!(updated.rune_amount, RUNE_AMOUNT);
        assert_eq!(updated.rune_id, "rune_2");
        assert_eq!(updated.status, UtxoStatus::Confirmed);

        Ok(())
    }

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn test_update_status(db: sqlx::PgPool) -> Result<(), DbError> {
        let repo = make_test_repo(db);
        cleanup_test_db(&repo).await;

        let utxo = create_test_utxo(
            "0000000000000000000000000000000000000000000000000000000000000003",
            0,
            RUNE_AMOUNT,
            "test_rune",
            UtxoStatus::Confirmed,
        );
        repo.insert_utxo(utxo.clone()).await?;

        repo.update_status(utxo.outpoint, UtxoStatus::Spent).await?;

        let updated = repo.get_utxo(utxo.outpoint).await?.unwrap();
        assert_eq!(updated.status, UtxoStatus::Spent);

        Ok(())
    }

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn test_update_status_not_found(db: sqlx::PgPool) -> Result<(), DbError> {
        let repo = make_test_repo(db);
        cleanup_test_db(&repo).await;

        let outpoint = OutPoint {
            txid: Txid::from_str("0000000000000000000000000000000000000000000000000000000000000999").unwrap(),
            vout: 0,
        };

        let result = repo.update_status(outpoint, UtxoStatus::Spent).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), DbError::NotFound(_)));

        Ok(())
    }

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn test_list_unspent(db: sqlx::PgPool) -> Result<(), DbError> {
        let repo = make_test_repo(db);
        cleanup_test_db(&repo).await;

        let utxos = vec![
            create_test_utxo(
                "0000000000000000000000000000000000000000000000000000000000000010",
                0,
                RUNE_AMOUNT,
                "rune_a",
                UtxoStatus::Confirmed,
            ),
            create_test_utxo(
                "0000000000000000000000000000000000000000000000000000000000000011",
                0,
                RUNE_AMOUNT,
                "rune_a",
                UtxoStatus::Pending,
            ),
            create_test_utxo(
                "0000000000000000000000000000000000000000000000000000000000000012",
                0,
                RUNE_AMOUNT,
                "rune_a",
                UtxoStatus::Spent,
            ),
            create_test_utxo(
                "0000000000000000000000000000000000000000000000000000000000000013",
                0,
                RUNE_AMOUNT,
                "rune_b",
                UtxoStatus::Confirmed,
            ),
        ];

        for utxo in utxos {
            repo.insert_utxo(utxo).await?;
        }

        let unspent = repo.list_unspent("rune_a".to_string()).await?;
        assert_eq!(unspent.len(), 2);

        let statuses: Vec<UtxoStatus> = unspent.iter().map(|u| u.status).collect();
        assert!(statuses.contains(&UtxoStatus::Confirmed));
        assert!(statuses.contains(&UtxoStatus::Pending));
        assert!(!statuses.contains(&UtxoStatus::Spent));

        Ok(())
    }

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn test_select_utxos_for_amount(db: sqlx::PgPool) -> Result<(), DbError> {
        let repo = make_test_repo(db);
        cleanup_test_db(&repo).await;

        let utxos = vec![
            create_test_utxo(
                "0000000000000000000000000000000000000000000000000000000000000020",
                0,
                100,
                "rune_select",
                UtxoStatus::Confirmed,
            ),
            create_test_utxo(
                "0000000000000000000000000000000000000000000000000000000000000021",
                0,
                200,
                "rune_select",
                UtxoStatus::Confirmed,
            ),
            create_test_utxo(
                "0000000000000000000000000000000000000000000000000000000000000022",
                0,
                300,
                "rune_select",
                UtxoStatus::Confirmed,
            ),
        ];

        for utxo in utxos {
            repo.insert_utxo(utxo).await?;
        }

        let selected = repo.select_utxos_for_amount("rune_select".to_string(), 250).await?;

        assert_eq!(selected.len(), 2);
        let total: u64 = selected.iter().map(|u| u.rune_amount).sum();
        assert!(total >= 250);

        for utxo in &selected {
            assert_eq!(utxo.status, UtxoStatus::Spent);
        }

        Ok(())
    }

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn test_select_utxos_insufficient_funds(db: sqlx::PgPool) -> Result<(), DbError> {
        let repo = make_test_repo(db);
        cleanup_test_db(&repo).await;

        let utxo = create_test_utxo(
            "0000000000000000000000000000000000000000000000000000000000000030",
            0,
            100,
            "rune_insufficient",
            UtxoStatus::Confirmed,
        );
        repo.insert_utxo(utxo).await?;

        let result = repo.select_utxos_for_amount("rune_insufficient".to_string(), 500).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), DbError::BadRequest(_)));

        let unspent = repo.list_unspent("rune_insufficient".to_string()).await?;
        assert_eq!(unspent.len(), 1);
        assert_eq!(unspent[0].status, UtxoStatus::Confirmed);

        Ok(())
    }

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn test_get_utxo(db: sqlx::PgPool) -> Result<(), DbError> {
        let repo = make_test_repo(db);
        cleanup_test_db(&repo).await;

        let utxo = create_test_utxo(
            "0000000000000000000000000000000000000000000000000000000000000040",
            5,
            RUNE_AMOUNT,
            "rune_get",
            UtxoStatus::Pending,
        );
        repo.insert_utxo(utxo.clone()).await?;

        let retrieved = repo.get_utxo(utxo.outpoint).await?.unwrap();
        assert_eq!(retrieved.outpoint, utxo.outpoint);
        assert_eq!(retrieved.rune_amount, RUNE_AMOUNT);
        assert_eq!(retrieved.rune_id, "rune_get");

        Ok(())
    }

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn test_get_utxo_not_found(db: sqlx::PgPool) -> Result<(), DbError> {
        let repo = make_test_repo(db);
        cleanup_test_db(&repo).await;

        let outpoint = OutPoint {
            txid: Txid::from_str("0000000000000000000000000000000000000000000000000000000000000888").unwrap(),
            vout: 0,
        };

        let result = repo.get_utxo(outpoint).await?;
        assert!(result.is_none());

        Ok(())
    }

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn test_delete_utxo(db: sqlx::PgPool) -> Result<(), DbError> {
        let repo = make_test_repo(db);
        cleanup_test_db(&repo).await;

        let utxo = create_test_utxo(
            "0000000000000000000000000000000000000000000000000000000000000050",
            0,
            RUNE_AMOUNT,
            "rune_delete",
            UtxoStatus::Confirmed,
        );
        repo.insert_utxo(utxo.clone()).await?;

        repo.delete_utxo(utxo.outpoint).await?;

        let result = repo.get_utxo(utxo.outpoint).await?;
        assert!(result.is_none());

        Ok(())
    }

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn test_delete_utxo_not_found(db: sqlx::PgPool) -> Result<(), DbError> {
        let repo = make_test_repo(db);
        cleanup_test_db(&repo).await;

        let outpoint = OutPoint {
            txid: Txid::from_str("0000000000000000000000000000000000000000000000000000000000000777").unwrap(),
            vout: 0,
        };

        let result = repo.delete_utxo(outpoint).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), DbError::NotFound(_)));

        Ok(())
    }

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn test_update_sats_fee_amount(db: sqlx::PgPool) -> Result<(), DbError> {
        let repo = make_test_repo(db);
        cleanup_test_db(&repo).await;

        let utxo = create_test_utxo(
            "0000000000000000000000000000000000000000000000000000000000000060",
            0,
            RUNE_AMOUNT,
            "rune_fee",
            UtxoStatus::Confirmed,
        );
        repo.insert_utxo(utxo.clone()).await?;

        repo.update_sats_fee_amount(utxo.outpoint, RUNE_AMOUNT).await?;

        let updated = repo.get_utxo(utxo.outpoint).await?.unwrap();
        assert_eq!(updated.sats_amount, RUNE_AMOUNT);

        Ok(())
    }

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn test_get_utxo_by_btc_address(db: sqlx::PgPool) -> Result<(), DbError> {
        let repo = make_test_repo(db);
        cleanup_test_db(&repo).await;

        let address = "bcrt1qw508d6qejxtdg4y5r3zarvary0c5xw7kygt080";
        let utxo = create_test_utxo(
            "0000000000000000000000000000000000000000000000000000000000000070",
            0,
            RUNE_AMOUNT,
            "rune_addr",
            UtxoStatus::Confirmed,
        );
        repo.insert_utxo(utxo.clone()).await?;

        let found = repo.get_utxo_by_btc_address(address.to_string()).await?.unwrap();
        assert_eq!(found.outpoint, utxo.outpoint);
        assert_eq!(found.btc_address.to_string(), address);

        Ok(())
    }

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn test_concurrent_select_utxos(db: sqlx::PgPool) -> Result<(), DbError> {
        let repo = make_test_repo(db);
        cleanup_test_db(&repo).await;

        let utxos = vec![
            create_test_utxo(
                "0000000000000000000000000000000000000000000000000000000000000080",
                0,
                500,
                "rune_concurrent",
                UtxoStatus::Confirmed,
            ),
            create_test_utxo(
                "0000000000000000000000000000000000000000000000000000000000000081",
                0,
                500,
                "rune_concurrent",
                UtxoStatus::Confirmed,
            ),
        ];

        for utxo in utxos {
            repo.insert_utxo(utxo).await?;
        }

        let f1 = repo.select_utxos_for_amount("rune_concurrent".to_string(), 800);
        let f2 = repo.select_utxos_for_amount("rune_concurrent".to_string(), 800);

        let (r1, r2) = tokio::join!(f1, f2);

        let success_count = [r1.as_ref(), r2.as_ref()].iter().filter(|r| r.is_ok()).count();

        assert!(success_count >= 1, "At least one should succeed");

        Ok(())
    }

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn test_comprehensive_flow(db: sqlx::PgPool) -> Result<(), DbError> {
        let repo = make_test_repo(db);
        cleanup_test_db(&repo).await;

        let utxos = vec![
            create_test_utxo(
                "0000000000000000000000000000000000000000000000000000000000000090",
                0,
                100,
                "flow_rune",
                UtxoStatus::Confirmed,
            ),
            create_test_utxo(
                "0000000000000000000000000000000000000000000000000000000000000091",
                0,
                200,
                "flow_rune",
                UtxoStatus::Confirmed,
            ),
            create_test_utxo(
                "0000000000000000000000000000000000000000000000000000000000000092",
                0,
                300,
                "flow_rune",
                UtxoStatus::Pending,
            ),
        ];

        for utxo in &utxos {
            repo.insert_utxo(utxo.clone()).await?;
        }

        let unspent = repo.list_unspent("flow_rune".to_string()).await?;
        assert_eq!(unspent.len(), 3);

        let selected = repo.select_utxos_for_amount("flow_rune".to_string(), 250).await?;
        assert!(selected.len() >= 2);

        let change_utxo = create_test_utxo(
            "0000000000000000000000000000000000000000000000000000000000000093",
            0,
            50,
            "flow_rune",
            UtxoStatus::Pending,
        );
        repo.insert_utxo(change_utxo).await?;

        let final_unspent = repo.list_unspent("flow_rune".to_string()).await?;
        assert!(final_unspent.len() >= 1);

        Ok(())
    }
}
