mod utils;

mod test_btc_indexer_requests {
    use crate::utils::{tx_tracking_requests_vec_eq, TEST_LOGGER};
    use bitcoin::{OutPoint, Txid};
    use btc_indexer_api::api::{Amount, BtcTxReview, TrackTxRequest};
    use global_utils::common_types::{get_uuid, TxIdWrapped, UrlWrapped};
    use local_db_store_indexer::init::LocalDbStorage;
    use local_db_store_indexer::schemas::track_tx_requests_storage::{
        TrackedReqStatus, TxRequestsTrackingStorageTrait, TxTrackingRequestsToSendResponse,
    };
    use local_db_store_indexer::schemas::tx_tracking_storage::{TxToUpdateStatus, TxTrackingStorageTrait};
    use ordinals::RuneId;
    use persistent_storage::init::PostgresPool;
    use std::str::FromStr;
    use titan_client::{Transaction, TransactionStatus};
    use url::Url;

    pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

    const RUNE_AMOUNT: Amount = 45667;
    const OUTPOINT_VOUT: u32 = 123;
    const SIZE_TITAN_TX: u64 = 12334;
    const WEIGHT_TITAN_TX: u64 = 4321;
    const BLOCK_HEIGHT: u64 = 111;

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn test_one_inserting(pool: PostgresPool) -> anyhow::Result<()> {
        dotenv::dotenv()?;
        let _logger_guard = &*TEST_LOGGER;
        let storage = LocalDbStorage {
            postgres_repo: persistent_storage::init::PostgresRepo { pool }.into_shared(),
        };

        let uuid = get_uuid();
        let tx_id = Txid::from_str("06b6af9af2e1708335add6c5e99f5ed03e26f3392ce2a3325a3aa7d5588a3983")?;
        let outpoint = OutPoint {
            txid: tx_id.clone(),
            vout: OUTPOINT_VOUT,
        };
        let request = TrackTxRequest {
            callback_url: UrlWrapped(Url::from_str("https://example.com/callback")?),
            btc_address: "bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh".to_string(),
            out_point: outpoint,
            rune_id: RuneId::from_str("840000:3")?,
            rune_amount: RUNE_AMOUNT,
        };
        storage.track_tx_request(uuid, &request).await?;

        let get_req = storage.get_txs_to_update_status().await?;
        assert_eq!(
            get_req,
            vec![TxToUpdateStatus {
                tx_id: TxIdWrapped(tx_id.clone()),
                v_out: outpoint.vout,
                amount: request.rune_amount,
                rune_id: RuneId::from_str("840000:3")?,
            }]
        );

        let review = BtcTxReview::Success;
        let titan_tx = Transaction {
            txid: tx_id,
            version: 0,
            lock_time: 0,
            input: vec![],
            output: vec![],
            status: TransactionStatus {
                confirmed: true,
                block_height: Some(BLOCK_HEIGHT),
                block_hash: None,
            },
            size: SIZE_TITAN_TX,
            weight: WEIGHT_TITAN_TX,
        };
        storage.insert_tx_tracking_report(outpoint, &review, &titan_tx).await?;
        let get_req = storage.get_txs_to_update_status().await?;
        assert!(get_req.is_empty());

        let get_req = storage.get_values_to_send_response().await?;
        assert!(
            tx_tracking_requests_vec_eq(
                &get_req,
                &vec![TxTrackingRequestsToSendResponse {
                    uuid,
                    out_point: outpoint,
                    callback_url: request.callback_url,
                    review,
                    transaction: titan_tx,
                }]
            ),
            "TxTrackingRequestsToSendResponse vectors are not equal"
        );

        let _ = storage
            .finalize_tx_request(get_req[0].uuid, TrackedReqStatus::Finished)
            .await?;

        let get_req = storage.get_values_to_send_response().await?;
        assert!(get_req.is_empty());

        Ok(())
    }
}
