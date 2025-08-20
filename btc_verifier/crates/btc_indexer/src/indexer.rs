use std::{collections::HashMap, sync::Arc, time::Duration};

use async_trait::async_trait;
use bitcoincore_rpc::{Client, RawTx, RpcApi, bitcoin, json};
use config_parser::config::{BtcIndexerParams, BtcRpcCredentials};
use persistent_storage::init::PersistentRepoShared;
use titan_client::{Event, EventType, TitanApi, TitanClient};
use titan_types::{AddressTxOut, Transaction};
use tokio::sync::{
    RwLock,
    mpsc::{Receiver, Sender},
    oneshot,
};
use tokio_util::sync::CancellationToken;
use tracing::{error, log::debug};

use crate::api::{AccountReplenishmentEvent, BtcIndexerApi};
type SubscriptionStorage = Arc<RwLock<HashMap<EventType, EventTypeChannels>>>;
#[derive()]
pub struct BtcIndexer<C> {
    btc_indexer_params: BtcIndexerParams,
    //todo: maybe move into traits?
    // subscription_storage: Arc<RwLock<HashMap<EventType, EventTypeChannels>>>,
    indexer_client: C,
    btc_core: Arc<Client>,
    cancellation_token: CancellationToken,
}

pub struct EventTypeChannels {
    subscription_emitter: Receiver<Event>,
    subscription_recipient: Sender<Event>,
}

pub struct IndexerParamsWithApi<C> {
    indexer_params: IndexerParams,
    titan_api_client: C,
}

pub struct IndexerParams {
    btc_rpc_creds: BtcRpcCredentials,
    db_pool: PersistentRepoShared,
    btc_indexer_params: BtcIndexerParams,
}

impl BtcIndexer<TitanClient> {
    pub fn with_api(params: IndexerParams) -> crate::error::Result<Self> {
        let titan_api_client = titan_client::TitanClient::new(&params.btc_rpc_creds.url.to_string());
        Self::new(IndexerParamsWithApi {
            indexer_params: params,
            titan_api_client,
        })
    }
}

impl<C: TitanApi> BtcIndexer<C> {
    pub fn new(params: IndexerParamsWithApi<C>) -> crate::error::Result<Self> {
        let cancellation_token = CancellationToken::new();
        let btc_rpc_client = Arc::new(Client::new(
            &params.indexer_params.btc_rpc_creds.url.to_string(),
            params.indexer_params.btc_rpc_creds.get_btc_creds(),
        )?);
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
            'checking_loop: loop {
                tokio::select! {
                    _ = cancellation_token.cancelled() => {
                        debug!("[Btc indexer] Closing [Btc indexer] account updating task, because of cancellation token");
                        break 'checking_loop;
                    },
                    _ = interval.tick() => {
                        match titan_client.get_address(&account_addr).await{
                            Ok(data) => {
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
    }

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
    }
}

#[async_trait]
impl<C: TitanApi> BtcIndexerApi for BtcIndexer<C> {
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

    fn get_tx_info(&self, tx_id: bitcoin::Txid) -> crate::error::Result<bitcoin::transaction::Transaction> {
        Ok(self.btc_core.get_by_id(&tx_id)?)
    }

    fn get_blockchain_info(&self) -> crate::error::Result<json::GetBlockchainInfoResult> {
        Ok(self.btc_core.get_blockchain_info()?)
    }

    fn broadcast_transaction(&self, tx: impl RawTx) -> crate::error::Result<bitcoin::blockdata::transaction::Txid> {
        Ok(self.btc_core.send_raw_transaction(tx)?)
    }
}

impl<C> Drop for BtcIndexer<C> {
    fn drop(&mut self) {
        self.cancellation_token.cancel()
    }
}

#[cfg(test)]
mod testing {
    use std::str::FromStr;

    use bitcoincore_rpc::{RawTx, bitcoin::Txid};
    use config_parser::config::{BtcRpcCredentials, ConfigVariant, PostgresDbCredentials, ServerConfig};
    use ordinals::Runestone;
    use persistent_storage::init::PostgresRepo;

    use crate::{
        api::BtcIndexerApi,
        indexer::{BtcIndexer, IndexerParams},
    };

    #[tokio::test]
    async fn init_btc_indexer() -> anyhow::Result<()> {
        dotenv::dotenv()?;
        let btc_rpc_creds = BtcRpcCredentials::new()?;
        let db_pool = PostgresRepo::from_config(PostgresDbCredentials::new()?)
            .await?
            .into_shared();
        let app_config = ServerConfig::init_config(ConfigVariant::Local)?;
        let indexer = BtcIndexer::with_api(IndexerParams {
            btc_rpc_creds,
            db_pool,
            btc_indexer_params: app_config.btc_indexer_config,
        })?;
        println!("Blockchain info: {:?}", indexer.get_blockchain_info()?);
        Ok(())
    }

    #[tokio::test]
    async fn get_btc_tx_by_id() -> anyhow::Result<()> {
        dotenv::dotenv()?;
        let btc_rpc_creds = BtcRpcCredentials::new()?;
        let db_pool = PostgresRepo::from_config(PostgresDbCredentials::new()?)
            .await?
            .into_shared();
        let app_config = ServerConfig::init_config(ConfigVariant::Local)?;
        let indexer = BtcIndexer::with_api(IndexerParams {
            btc_rpc_creds,
            db_pool,
            btc_indexer_params: app_config.btc_indexer_config,
        })?;
        let tx_info = indexer.get_tx_info(Txid::from_str(
            "250f0473c42878dbe9153100100c9c9a55ea85eea688fd358d975351b33d2741",
        )?)?;
        println!("Blockchain info: {:?}", tx_info);
        println!("Blockchain info: {:?}", tx_info.raw_hex());
        println!("Blockchain info: {:?}", tx_info.tx_out(1)?.script_pubkey.as_script());
        let hex = "020704a7d987f890dd81b7f4ebe7d07b0101052406000ae80708904e";
        let etching = Runestone::decipher(&tx_info);
        println!("Parsed ordinals: {:?}", etching);
        // println!("Parsed ordinals: {:?}",ordinals::Etching::deserialize("020704a7d987f890dd81b7f4ebe7d07b0101052406000ae80708904e")?);

        Ok(())
    }
}
