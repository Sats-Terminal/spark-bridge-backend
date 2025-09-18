use std::{sync::Arc, time::Duration};

use crate::api::{AccountReplenishmentEvent, BtcIndexerApi, ChanMsg};
use async_trait::async_trait;
use bitcoin::OutPoint;
use bitcoincore_rpc::{Client, RawTx, RpcApi, bitcoin, json};
use btc_indexer_api::api::{Amount, ResponseMeta, TrackTxResponse, VOut};
use config_parser::config::{BtcIndexerParams, BtcRpcCredentials};
use global_utils::common_resp::Empty;
use local_db_store_indexer::{
    PersistentRepoTrait,
    init::LocalDbStorage,
    schemas::runes_spark::btc_indexer_work_checkpoint::{BtcIndexerWorkCheckpoint, StatusBtcIndexer, Update},
};
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

pub struct BtcIndexer<C, Db> {
    pub btc_indexer_params: BtcIndexerParams,
    //todo: maybe move into traits?
    persistent_storage: Db,
    indexer_client: C,
    btc_core: Arc<Client>,
    cancellation_token: CancellationToken,
    chan_to_send_to: UnboundedSender<ChanMsg>,
}

pub struct IndexerParamsWithApi<C, Db> {
    pub indexer_params: IndexerParams<Db>,
    pub titan_api_client: C,
}

pub struct IndexerParams<Db> {
    pub btc_rpc_creds: BtcRpcCredentials,
    pub db_pool: Db,
    pub btc_indexer_params: BtcIndexerParams,
}

impl BtcIndexer<TitanClient, LocalDbStorage> {
    #[instrument(skip(params))]
    pub fn with_api(params: IndexerParams<LocalDbStorage>) -> crate::error::Result<Self> {
        let titan_api_client = TitanClient::new(&params.btc_rpc_creds.url.to_string());
        Self::new(IndexerParamsWithApi {
            indexer_params: params,
            titan_api_client,
        })
    }
}

impl<C: Clone, Db: Clone> Clone for BtcIndexer<C, Db> {
    fn clone(&self) -> Self {
        BtcIndexer {
            btc_indexer_params: self.btc_indexer_params.clone(),
            persistent_storage: self.persistent_storage.clone(),
            indexer_client: self.indexer_client.clone(),
            btc_core: self.btc_core.clone(),
            cancellation_token: self.cancellation_token.clone(),
            chan_to_send_to: self.chan_to_send_to.clone(),
        }
    }
}

impl<C: TitanApi, Db: PersistentRepoTrait + Send + Sync + Clone + 'static> BtcIndexer<C, Db> {
    #[instrument(skip(params))]
    pub fn new(params: IndexerParamsWithApi<C, Db>) -> crate::error::Result<Self> {
        let cancellation_token = CancellationToken::new();
        let (chan_sender, chan_receiver) = tokio::sync::mpsc::unbounded_channel::<ChanMsg>();
        // spawn task to track given tx_ids
        crate::tx_tracking_task::spawn(
            cancellation_token.clone(),
            params.indexer_params.db_pool.clone(),
            params.indexer_params.btc_indexer_params,
            params.titan_api_client.clone(),
            chan_receiver,
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
            btc_core: btc_rpc_client,
            chan_to_send_to: chan_sender,
            cancellation_token,
        };
        Ok(indexer)
    }

    pub fn create_default_titan_api(btc_rpc_creds: BtcRpcCredentials) -> TitanClient {
        TitanClient::new(&btc_rpc_creds.url.to_string())
    }

    /// Sends appropriate msg to thread that updates statuses of transactions in BtcIndexer
    pub async fn send_new_tx_to_tracking_thread(
        &self,
        btc_address: String,
        out_point: OutPoint,
        amount: Amount,
    ) -> crate::error::Result<()> {
        self.chan_to_send_to.send(ChanMsg {
            btc_address,
            out_point,
            amount,
        })?;
        Ok(())
    }
}

#[async_trait]
impl<C: TitanApi, Db: PersistentRepoTrait> BtcIndexerApi for BtcIndexer<C, Db> {
    #[inline]
    #[instrument(level = "debug", skip(self))]
    async fn check_tx_changes(
        &self,
        out_point: OutPoint,
        amount: Amount,
    ) -> crate::error::Result<Option<ResponseMeta>> {
        //todo: check whether tx is in db

        // match self.indexer_client.get_transaction(&tx_id).await {
        //     Ok(data) => {
        //         if data.status.confirmed {
        //             //todo: write transaction into db and wait for strong confirmation
        //
        //             check_block_height() ;
        //         } else{
        //             // the transaction doesn't included in blockchain
        //             Ok(None)
        //         }
        //     }
        //     Err(e) => {
        //         warn!("[{TX_TRACKING_LOG_PATH}] Failed to retrieve information : {e}");
        //         Ok(None)
        //     }
        // }
        todo!()
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

impl<C, Db> Drop for BtcIndexer<C, Db> {
    #[instrument(skip(self))]
    fn drop(&mut self) {
        debug!("[{BTC_INDEXER_LOG_PATH}] Closing indexer");
        self.cancellation_token.cancel()
    }
}
