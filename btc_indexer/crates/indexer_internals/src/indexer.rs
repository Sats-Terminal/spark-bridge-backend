use std::sync::Arc;

use crate::api::BtcIndexerApi;
use crate::tx_arbiter::{TxArbiter, TxArbiterTrait};
use async_trait::async_trait;
use bitcoincore_rpc::{Client, RawTx, RpcApi, bitcoin, json};
use btc_indexer_api::api::TrackTxRequest;
use config_parser::config::{BtcIndexerParams, BtcRpcCredentials, TitanConfig};

use local_db_store_indexer::init::IndexerDbBounds;
use local_db_store_indexer::init::LocalDbStorage;

use crate::error::BtcIndexerError;
use titan_client::{TitanApi, TitanClient};
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tracing::instrument;
use uuid::Uuid;

pub struct BtcIndexer<C, Db, TxValidator> {
    pub btc_indexer_params: BtcIndexerParams,
    persistent_storage: Db,
    indexer_client: Arc<C>,
    tx_validator: Arc<TxValidator>,
    btc_core: Arc<Client>,
    cancellation_token: CancellationToken,
    task_tracker: TaskTracker,
}

pub struct IndexerParamsWithApi<C, Db, TxValidator> {
    pub indexer_params: IndexerParams<Db>,
    pub titan_api_client: Arc<C>,
    pub tx_validator: Arc<TxValidator>,
}

pub struct IndexerParams<Db> {
    pub titan_config: TitanConfig,
    pub btc_rpc_creds: BtcRpcCredentials,
    pub db_pool: Db,
    pub btc_indexer_params: BtcIndexerParams,
}

impl BtcIndexer<TitanClient, LocalDbStorage, TxArbiter> {
    pub fn with_api(params: IndexerParams<LocalDbStorage>) -> crate::error::Result<Self> {
        let titan_api_client = TitanClient::new(params.titan_config.url.as_ref());
        Self::new(IndexerParamsWithApi {
            indexer_params: params,
            titan_api_client: Arc::new(titan_api_client),
            tx_validator: Arc::new(TxArbiter {}),
        })
    }
}

impl<C: Clone, Db: Clone, TxValidator: Clone> Clone for BtcIndexer<C, Db, TxValidator> {
    fn clone(&self) -> Self {
        BtcIndexer {
            btc_indexer_params: self.btc_indexer_params,
            persistent_storage: self.persistent_storage.clone(),
            indexer_client: self.indexer_client.clone(),
            tx_validator: self.tx_validator.clone(),
            btc_core: self.btc_core.clone(),
            cancellation_token: self.cancellation_token.clone(),
            task_tracker: self.task_tracker.clone(),
        }
    }
}

impl<C: TitanApi, Db: IndexerDbBounds, TxValidator: TxArbiterTrait> BtcIndexer<C, Db, TxValidator> {
    pub fn new(params: IndexerParamsWithApi<C, Db, TxValidator>) -> crate::error::Result<Self> {
        let cancellation_token = CancellationToken::new();
        let mut task_tracker = TaskTracker::default();
        crate::tx_tracking_task::spawn(
            cancellation_token.clone(),
            params.indexer_params.db_pool.clone(),
            params.indexer_params.btc_indexer_params,
            params.titan_api_client.clone(),
            params.tx_validator.clone(),
            &mut task_tracker,
        );
        let btc_rpc_client = Arc::new(Client::new(
            &params.indexer_params.btc_rpc_creds.url.to_string(),
            params.indexer_params.btc_rpc_creds.get_btc_creds(),
        )?);
        tracing::debug!(
            "Initialization passed with configuration, {:?}",
            params.indexer_params.btc_indexer_params
        );
        let indexer = BtcIndexer {
            btc_indexer_params: params.indexer_params.btc_indexer_params,
            persistent_storage: params.indexer_params.db_pool,
            indexer_client: params.titan_api_client,
            tx_validator: params.tx_validator,
            btc_core: btc_rpc_client,
            cancellation_token,
            task_tracker,
        };
        Ok(indexer)
    }

    pub fn create_default_titan_api(btc_rpc_creds: BtcRpcCredentials) -> TitanClient {
        TitanClient::new(&btc_rpc_creds.url.to_string())
    }
}

#[async_trait]
impl<C: TitanApi, Db: IndexerDbBounds, TxValidator: TxArbiterTrait> BtcIndexerApi for BtcIndexer<C, Db, TxValidator> {
    #[inline]
    #[instrument(level = "trace", skip(self), ret)]
    async fn check_tx_changes(&self, uuid: Uuid, payload: &TrackTxRequest) -> crate::error::Result<()> {
        self.persistent_storage.track_tx_request(uuid, payload).await?;
        Ok(())
    }

    #[instrument(level = "trace", skip(self), err)]
    async fn healthcheck(&self) -> crate::error::Result<()> {
        if self.task_tracker.is_closed() {
            return Err(BtcIndexerError::HealthcheckError(
                "Threads closed, check internal logic".to_string(),
            ));
        }
        self.persistent_storage
            .healthcheck()
            .await
            .map_err(|e| BtcIndexerError::HealthcheckError(e.to_string()))?;
        let _ = self.indexer_client.get_status().await.map_err(|e| {
            BtcIndexerError::HealthcheckError(format!("Unable to retrieve titan indexer status, err: {e}"))
        });
        Ok(())
    }

    // #[instrument(level = "trace", skip(self))]
    // fn get_tx_info(&self, tx_id: bitcoin::Txid) -> crate::error::Result<bitcoin::transaction::Transaction> {
    //     Ok(self.btc_core.get_by_id(&tx_id)?)
    // }

    #[instrument(level = "trace", skip(self))]
    fn get_blockchain_info(&self) -> crate::error::Result<json::GetBlockchainInfoResult> {
        Ok(self.btc_core.get_blockchain_info()?)
    }

    #[instrument(level = "trace", skip(self, tx))]
    fn broadcast_transaction(&self, tx: impl RawTx) -> crate::error::Result<bitcoin::blockdata::transaction::Txid> {
        let hex_tx = tx.raw_hex();
        tracing::info!("broadcasting transaction, {hex_tx}");
        Ok(self.btc_core.send_raw_transaction(hex_tx)?)
    }
}

impl<C, Db, TxValidator> Drop for BtcIndexer<C, Db, TxValidator> {
    #[instrument(skip(self))]
    fn drop(&mut self) {
        tracing::info!("Closing indexer");
        self.cancellation_token.cancel();
        self.task_tracker.close();
    }
}
