mod tests {
    use gateway_local_db_store::storage::LocalDbStorage;
    use gateway_session_storage::tracker::SessionTracker;
    use gateway_session_storage::traits::{RequestType, SessionStatus, SessionStorage};
    use global_utils::common_types::get_uuid;
    use persistent_storage::error::DbError as DatabaseError;
    use persistent_storage::init::{PostgresPool, PostgresRepo};
    use serde_json::json;
    use std::sync::Arc;

    async fn make_repo(db: PostgresPool) -> Arc<LocalDbStorage> {
        Arc::new(LocalDbStorage {
            postgres_repo: PostgresRepo { pool: db },
        })
    }

    async fn cleanup_sessions(repo: Arc<LocalDbStorage>) {
        sqlx::query("TRUNCATE gateway.session_requests")
            .execute(&repo.get_conn().await.unwrap())
            .await
            .unwrap();
    }

    pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn test_create_session(db: PostgresPool) -> Result<(), DatabaseError> {
        let repo = make_repo(db).await;
        //setup_test_table(&repo).await;
        cleanup_sessions(repo.clone()).await;

        let request_data = json!({
            "rune_id": "test_rune_123",
            "amount": 1000,
            "recipient": "test_address"
        });

        let session_id = repo
            .create_session(RequestType::SendRunes, request_data.clone())
            .await?;

        assert_eq!(session_id.to_string().len(), 36);

        let session = repo.get_session(session_id).await?;
        assert_eq!(session.session_id, session_id);
        assert_eq!(session.request_type, RequestType::SendRunes);
        assert_eq!(session.status, SessionStatus::Pending);
        assert_eq!(session.request, request_data);
        assert!(session.response.is_none());
        assert!(session.error.is_none());

        Ok(())
    }

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn test_session_status_updates(db: PostgresPool) -> Result<(), DatabaseError> {
        let repo = make_repo(db).await;
        //setup_test_table(&repo).await;
        cleanup_sessions(repo.clone()).await;

        let request_data = json!({"test": "data"});
        let session_id = repo
            .create_session(RequestType::CreateTransaction, request_data)
            .await?;

        repo.update_session_status(session_id, SessionStatus::InProgress)
            .await?;

        let session = repo.get_session(session_id).await?;
        assert_eq!(session.status, SessionStatus::InProgress);

        repo.set_session_error(session_id, "Test error occurred").await?;

        let session = repo.get_session(session_id).await?;
        assert_eq!(session.status, SessionStatus::Failed);
        assert_eq!(session.error, Some("Test error occurred".to_string()));

        Ok(())
    }

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn test_session_success(db: PostgresPool) -> Result<(), DatabaseError> {
        let repo = make_repo(db).await;
        //setup_test_table(&repo).await;
        cleanup_sessions(repo.clone()).await;

        let request_data = json!({"action": "test"});
        let session_id = repo
            .create_session(RequestType::BroadcastTransaction, request_data)
            .await?;

        let response_data = json!({
            "txid": "abc123def456",
            "block_height": 800000
        });

        repo.set_session_success(session_id, response_data.clone()).await?;

        let session = repo.get_session(session_id).await?;
        assert_eq!(session.status, SessionStatus::Success);
        assert_eq!(session.response, Some(response_data));
        assert!(session.error.is_none());

        Ok(())
    }

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn test_list_sessions(db: PostgresPool) -> Result<(), DatabaseError> {
        let repo = make_repo(db).await;
        //setup_test_table(&repo).await;
        cleanup_sessions(repo.clone()).await;

        let session1 = repo
            .create_session(RequestType::SendRunes, json!({"test": "1"}))
            .await?;

        let session2 = repo
            .create_session(RequestType::CreateTransaction, json!({"test": "2"}))
            .await?;

        repo.update_session_status(session1, SessionStatus::Success).await?;
        repo.update_session_status(session2, SessionStatus::Failed).await?;

        let all_sessions = repo.list_sessions(Some(10), None).await?;
        assert_eq!(all_sessions.len(), 2);

        let success_sessions = repo.list_sessions_by_status(SessionStatus::Success, Some(10)).await?;
        assert_eq!(success_sessions.len(), 1);
        assert_eq!(success_sessions[0].session_id, session1);

        let failed_sessions = repo.list_sessions_by_status(SessionStatus::Failed, Some(10)).await?;
        assert_eq!(failed_sessions.len(), 1);
        assert_eq!(failed_sessions[0].session_id, session2);

        Ok(())
    }

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn test_session_tracker_helper(db: PostgresPool) -> Result<(), DatabaseError> {
        let repo = make_repo(db).await;
        //setup_test_table(&repo).await;
        cleanup_sessions(repo.clone()).await;

        let tracker = SessionTracker { repo: repo.clone() };

        let session_id = tracker
            .start_session(
                RequestType::GenerateFrostSignature,
                json!({"message": "test_signature"}),
            )
            .await?;

        let status = tracker.get_session_status(session_id).await?;
        assert!(matches!(status, SessionStatus::InProgress));

        let result_data = json!({"signature": "abcd1234"});
        tracker.complete_session(session_id, result_data.clone()).await?;

        let session = repo.get_session(session_id).await?;
        assert_eq!(session.status, SessionStatus::Success);
        assert_eq!(session.response, Some(result_data));

        Ok(())
    }

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn test_session_not_found(db: PostgresPool) -> Result<(), DatabaseError> {
        let repo = make_repo(db).await;
        //setup_test_table(&repo).await;
        cleanup_sessions(repo.clone()).await;

        let non_existent_id = get_uuid();

        let result = repo.get_session(non_existent_id).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), DatabaseError::NotFound(_)));

        Ok(())
    }

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn test_concurrent_session_operations(db: PostgresPool) -> Result<(), DatabaseError> {
        let repo = make_repo(db).await;
        //setup_test_table(&repo).await;
        cleanup_sessions(repo.clone()).await;

        let handles: Vec<_> = (0..5)
            .map(|i| {
                let repo_clone = repo.clone();
                tokio::spawn(async move {
                    repo_clone
                        .create_session(RequestType::SendRunes, json!({"batch_id": i}))
                        .await
                })
            })
            .collect();

        let results: Result<Vec<_>, _> = futures::future::try_join_all(handles).await;
        let session_ids: Result<Vec<_>, _> = results.unwrap().into_iter().collect();
        let session_ids = session_ids?;

        assert_eq!(session_ids.len(), 5);

        let mut unique_ids = std::collections::HashSet::new();
        for id in &session_ids {
            assert!(unique_ids.insert(*id), "Duplicate session ID found");
        }

        let all_sessions = repo.list_sessions(Some(10), None).await?;
        assert_eq!(all_sessions.len(), 5);

        Ok(())
    }

    #[tokio::test]
    async fn test_request_type_enum_conversion() {
        let types = vec![
            RequestType::SendRunes,
            RequestType::CreateTransaction,
            RequestType::BroadcastTransaction,
            RequestType::GenerateFrostSignature,
        ];

        for original_type in types {
            let type_string = original_type;
            let converted_back = RequestType::from(type_string.clone());

            match (&original_type, &converted_back) {
                (RequestType::SendRunes, RequestType::SendRunes) => {}
                (RequestType::CreateTransaction, RequestType::CreateTransaction) => {}
                (RequestType::BroadcastTransaction, RequestType::BroadcastTransaction) => {}
                (RequestType::GenerateFrostSignature, RequestType::GenerateFrostSignature) => {}
                _ => panic!("Type conversion failed for {:?}", type_string),
            }
        }
    }
}
