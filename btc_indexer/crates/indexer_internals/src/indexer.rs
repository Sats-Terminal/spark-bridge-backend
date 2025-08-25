use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use bitcoincore_rpc::{Client, RawTx, RpcApi, bitcoin, json};
use config_parser::config::{BtcIndexerParams, BtcRpcCredentials};
use persistent_storage::init::PersistentRepoShared;
use titan_client::{TitanApi, TitanClient};
use titan_types::{AddressTxOut, Transaction};
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;
use tracing::{error, instrument, log::debug, trace};

use crate::api::{AccountReplenishmentEvent, BtcIndexerApi};

const BTC_INDEXER_LOG_PATH: &str = "btc_indexer";
const TX_TRACKING_LOG_PATH: &str = "btc_indexer:tx_tracking";
const ACCOUNT_TRACKING_LOG_PATH: &str = "btc_indexer:account_tracking";

pub struct BtcIndexer<C> {
    btc_indexer_params: BtcIndexerParams,
    //todo: maybe move into traits?
    // subscription_storage: Arc<RwLock<HashMap<EventType, EventTypeChannels>>>,
    indexer_client: C,
    btc_core: Arc<Client>,
    cancellation_token: CancellationToken,
}

pub struct IndexerParamsWithApi<C> {
    pub indexer_params: IndexerParams,
    pub titan_api_client: C,
}

pub struct IndexerParams {
    pub btc_rpc_creds: BtcRpcCredentials,
    pub db_pool: PersistentRepoShared,
    pub btc_indexer_params: BtcIndexerParams,
}

impl BtcIndexer<TitanClient> {
    #[instrument(skip(params))]
    pub fn with_api(params: IndexerParams) -> crate::error::Result<Self> {
        let titan_api_client = TitanClient::new(&params.btc_rpc_creds.url.to_string());
        Self::new(IndexerParamsWithApi {
            indexer_params: params,
            titan_api_client,
        })
    }
}

impl<C: Clone> Clone for BtcIndexer<C> {
    fn clone(&self) -> Self {
        BtcIndexer {
            btc_indexer_params: self.btc_indexer_params.clone(),
            indexer_client: self.indexer_client.clone(),
            btc_core: self.btc_core.clone(),
            cancellation_token: self.cancellation_token.clone(),
        }
    }
}

impl<C: TitanApi> BtcIndexer<C> {
    #[instrument(skip(params))]
    pub fn new(params: IndexerParamsWithApi<C>) -> crate::error::Result<Self> {
        let cancellation_token = CancellationToken::new();
        let btc_rpc_client = Arc::new(Client::new(
            &params.indexer_params.btc_rpc_creds.url.to_string(),
            params.indexer_params.btc_rpc_creds.get_btc_creds(),
        )?);
        debug!(
            "[Btc indexer] Initialization passed with configuration, {:?}",
            params.indexer_params.btc_indexer_params
        );
        Ok(BtcIndexer {
            btc_indexer_params: params.indexer_params.btc_indexer_params,
            // subscription_storage: storage,
            indexer_client: params.titan_api_client,
            btc_core: btc_rpc_client,
            cancellation_token,
        })
    }

    pub fn create_default_titan_api(btc_rpc_creds: BtcRpcCredentials) -> TitanClient {
        TitanClient::new(&btc_rpc_creds.url.to_string())
    }

    /// Spawns account replenishment tracking task
    #[instrument(level = "trace", skip(cancellation_token, titan_client), ret)]
    fn spawn_account_tracking_task(
        titan_client: C,
        event_tx: oneshot::Sender<AccountReplenishmentEvent>,
        account_addr: String,
        update_interval_millis: u64,
        cancellation_token: CancellationToken,
    ) {
        let mut interval = tokio::time::interval(Duration::from_millis(update_interval_millis));
        let is_confirmed_outs = |tx_outs: &[AddressTxOut]| -> bool { tx_outs.iter().all(|out| out.status.confirmed) };
        tokio::spawn(async move {
            // todo: save about user that begun transaction to renew connection in bad cases
            trace!("Loop spawned..");
            'checking_loop: loop {
                tokio::select! {
                    _ = cancellation_token.cancelled() => {
                        debug!("[Btc indexer] Closing [Btc indexer] account updating task, because of cancellation token");
                        break 'checking_loop;
                    },
                    _ = interval.tick() => {
                        trace!("[Btc indexer] tick triggered");
                        match titan_client.get_address(&account_addr).await{
                            Ok(data) => {
                                trace!("[Btc indexer] address data successfully received, {data:?}");
                                if  !data.outputs.is_empty() && is_confirmed_outs(&data.outputs) {
                                    let _ = event_tx.send(AccountReplenishmentEvent{address: account_addr,account_data:data});
                                    // todo: save data in db
                                    break 'checking_loop;
                                }
                            }
                            Err(e) => {
                                error!("[Btc indexer] Failed to retrieve account data by address: {e}")
                            }
                        };
                    }
                }
            }
        });
        trace!("[Btc indexer] Account tracking task");
    }

    #[instrument(level = "trace", skip(titan_client, event_tx), ret)]
    fn spawn_tx_tracking_task(
        titan_client: C,
        event_tx: oneshot::Sender<Transaction>,
        tx_id: bitcoin::Txid,
        update_interval_millis: u64,
        cancellation_token: CancellationToken,
    ) {
        let mut interval = tokio::time::interval(Duration::from_millis(update_interval_millis));
        tokio::spawn(async move {
            // todo: save about user that begun transaction to renew connection in bad cases
            'checking_loop: loop {
                tokio::select! {
                    _ = cancellation_token.cancelled() => {
                        debug!("[Btc indexer] Closing [Btc indexer] account updating task, because of cancellation token");
                        break 'checking_loop;
                    },
                    _ = interval.tick() => {
                        match titan_client.get_transaction(&tx_id).await{
                            Ok(data) => {
                                if data.status.confirmed {
                                    let _ = event_tx.send(data);
                                    // todo: save data in db
                                    break 'checking_loop;
                                }
                            }
                            Err(e) => {
                                error!("[Btc indexer] Failed to retrieve account data by address: {e}")
                            }
                        };
                    }
                }
            }
        });
        trace!("[Btc indexer] Transaction tracking task");
    }
}

#[async_trait]
impl<C: TitanApi> BtcIndexerApi for BtcIndexer<C> {
    #[instrument(level = "debug", skip(self))]
    fn track_tx_changes(&self, tx_id: bitcoin::Txid) -> crate::error::Result<oneshot::Receiver<Transaction>> {
        let (event_tx, event_rx) = oneshot::channel::<Transaction>();
        Self::spawn_tx_tracking_task(
            self.indexer_client.clone(),
            event_tx,
            tx_id,
            self.btc_indexer_params.update_interval_millis,
            self.cancellation_token.child_token(),
        );
        Ok(event_rx)
    }

    #[instrument(level = "debug", skip(self, account_id), fields(account_id=account_id.as_ref()))]
    fn track_account_changes(
        &self,
        account_id: impl AsRef<str>,
    ) -> crate::error::Result<oneshot::Receiver<AccountReplenishmentEvent>> {
        let tx_id = account_id.as_ref().to_string();
        let (event_tx, event_rx) = oneshot::channel::<AccountReplenishmentEvent>();
        Self::spawn_account_tracking_task(
            self.indexer_client.clone(),
            event_tx,
            tx_id,
            self.btc_indexer_params.update_interval_millis,
            self.cancellation_token.child_token(),
        );
        Ok(event_rx)
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

impl<C> Drop for BtcIndexer<C> {
    #[instrument(skip(self))]
    fn drop(&mut self) {
        debug!("[{BTC_INDEXER_LOG_PATH}] Closing indexer");
        self.cancellation_token.cancel()
    }
}
