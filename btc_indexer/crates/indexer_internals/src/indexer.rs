use std::{sync::Arc, time::Duration};

use crate::api::{AccountReplenishmentEvent, BtcIndexerApi};
use crate::tx_arbiter::{TxArbiter, TxArbiterTrait};
use async_trait::async_trait;
use bitcoin::OutPoint;
use bitcoincore_rpc::{Client, RawTx, RpcApi, bitcoin, json};
use btc_indexer_api::api::{Amount, ResponseMeta, TrackTxRequest, TrackTxResponse, VOut};
use config_parser::config::{BtcIndexerParams, BtcRpcCredentials};
use global_utils::common_resp::Empty;
use local_db_store_indexer::init::IndexerDbBounds;
use local_db_store_indexer::{PersistentRepoTrait, init::LocalDbStorage};
use sqlx::types::chrono::Utc;
use titan_client::{TitanApi, TitanClient};
use titan_types::{AddressTxOut, Transaction};
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, instrument, log::debug, trace, warn};
use uuid::Uuid;

const BTC_INDEXER_LOG_PATH: &str = "btc_indexer";
const TX_TRACKING_LOG_PATH: &str = "btc_indexer:tx_tracking";
const ACCOUNT_TRACKING_LOG_PATH: &str = "btc_indexer:account_tracking";

pub struct BtcIndexer<C, Db, TxValidator> {
    pub btc_indexer_params: BtcIndexerParams,
    //todo: maybe move into traits?
    persistent_storage: Db,
    indexer_client: C,
    tx_validator: TxValidator,
    btc_core: Arc<Client>,
    cancellation_token: CancellationToken,
}

pub struct IndexerParamsWithApi<C, Db, TxValidator> {
    pub indexer_params: IndexerParams<Db>,
    pub titan_api_client: C,
    pub tx_validator: TxValidator,
}

pub struct IndexerParams<Db> {
    pub btc_rpc_creds: BtcRpcCredentials,
    pub db_pool: Db,
    pub btc_indexer_params: BtcIndexerParams,
}

impl BtcIndexer<TitanClient, LocalDbStorage, TxArbiter> {
    #[instrument(skip(params))]
    pub fn with_api(params: IndexerParams<LocalDbStorage>) -> crate::error::Result<Self> {
        let titan_api_client = TitanClient::new(&params.btc_rpc_creds.url.to_string());
        Self::new(IndexerParamsWithApi {
            indexer_params: params,
            titan_api_client,
            tx_validator: TxArbiter {},
        })
    }
}

impl<C: Clone, Db: Clone, TxValidator: Clone> Clone for BtcIndexer<C, Db, TxValidator> {
    fn clone(&self) -> Self {
        BtcIndexer {
            btc_indexer_params: self.btc_indexer_params.clone(),
            persistent_storage: self.persistent_storage.clone(),
            indexer_client: self.indexer_client.clone(),
            tx_validator: self.tx_validator.clone(),
            btc_core: self.btc_core.clone(),
            cancellation_token: self.cancellation_token.clone(),
        }
    }
}

impl<C: TitanApi, Db: IndexerDbBounds, TxValidator: TxArbiterTrait> BtcIndexer<C, Db, TxValidator> {
    #[instrument(skip(params))]
    pub  fn new(params: IndexerParamsWithApi<C, Db, TxValidator>) -> crate::error::Result<Self> {
        let cancellation_token = CancellationToken::new();
        crate::tx_tracking_task::spawn(
            cancellation_token.clone(),
            params.indexer_params.db_pool.clone(),
            params.indexer_params.btc_indexer_params,
            params.titan_api_client.clone(),
            params.tx_validator.clone(),
        );
        let btc_rpc_client = Arc::new(Client::new(
            &params.indexer_params.btc_rpc_creds.url.to_string(),
            params.indexer_params.btc_rpc_creds.get_btc_creds(),
        )?);
        debug!(
            "[Btc indexer] Initialization passed with configuration, {:?}",
            params.indexer_params.btc_indexer_params
        );
        let indexer = BtcIndexer {
            btc_indexer_params: params.indexer_params.btc_indexer_params,
            persistent_storage: params.indexer_params.db_pool,
            indexer_client: params.titan_api_client,
            tx_validator: params.tx_validator,
            btc_core: btc_rpc_client,
            cancellation_token,
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
    #[instrument(level = "debug", skip(self), ret)]
    async fn check_tx_changes(&self, uuid: Uuid, payload: &TrackTxRequest) -> crate::error::Result<()> {
        self.persistent_storage.track_tx_request(uuid, &payload).await?;
        Ok(())
    }

    #[instrument(level = "debug", skip(self))]
    fn get_tx_info(&self, tx_id: bitcoin::Txid) -> crate::error::Result<bitcoin::transaction::Transaction> {
        Ok(self.btc_core.get_by_id(&tx_id)?)
    }

    #[instrument(level = "debug", skip(self))]
    fn get_blockchain_info(&self) -> crate::error::Result<json::GetBlockchainInfoResult> {
        Ok(self.btc_core.get_blockchain_info()?)
    }

    #[instrument(level = "debug", skip(self, tx))]
    fn broadcast_transaction(&self, tx: impl RawTx) -> crate::error::Result<bitcoin::blockdata::transaction::Txid> {
        let hex_tx = tx.raw_hex();
        trace!("[Btc indexer] broadcasting transaction, {hex_tx}");
        Ok(self.btc_core.send_raw_transaction(hex_tx)?)
    }
}

impl<C, Db, TxValidator> Drop for BtcIndexer<C, Db, TxValidator> {
    #[instrument(skip(self))]
    fn drop(&mut self) {
        debug!("[{BTC_INDEXER_LOG_PATH}] Closing indexer");
        self.cancellation_token.cancel()
    }
}
