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
use tracing::{error, instrument, log::debug, trace};

use crate::api::{AccountReplenishmentEvent, BtcIndexerApi};

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
    pub indexer_params: IndexerParams,
    pub titan_api_client: C,
}

pub struct IndexerParams {
    pub btc_rpc_creds: BtcRpcCredentials,
    pub db_pool: PersistentRepoShared,
    pub btc_indexer_params: BtcIndexerParams,
}

impl BtcIndexer<TitanClient> {
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
    fn drop(&mut self) {
        self.cancellation_token.cancel()
    }
}

#[cfg(test)]
mod testing {
    use std::{collections::HashMap, str::FromStr, sync::LazyLock};

    use async_trait::async_trait;
    use bitcoin::OutPoint;
    use bitcoincore_rpc::{RawTx, bitcoin::Txid};
    use config_parser::config::{BtcRpcCredentials, ConfigVariant, PostgresDbCredentials, ServerConfig};
    use global_utils::logger::{LoggerGuard, init_logger};
    use mockall::mock;
    use persistent_storage::init::PostgresRepo;
    use reqwest::header::HeaderMap;
    use titan_client::{Error, TitanApi};
    use titan_types::{
        AddressData, AddressTxOut, Block, BlockTip, InscriptionId, MempoolEntry, Pagination, PaginationResponse,
        RuneResponse, Status, Subscription, Transaction, TransactionStatus, TxOut, query,
    };

    static TEST_LOGGER: LazyLock<LoggerGuard> = LazyLock::new(|| init_logger());

    use crate::{
        api::BtcIndexerApi,
        indexer::{BtcIndexer, IndexerParams},
    };

    // Test requires to run Postgres & Docker files (bitcoind + titan)
    #[ignore]
    #[tokio::test]
    async fn init_btc_indexer() -> anyhow::Result<()> {
        dotenv::dotenv()?;
        let _logger_guard = &*TEST_LOGGER;
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

    mod mock_tests {
        use bitcoin::{BlockHash, hashes::Hash};
        use titan_types::{AddressTxOut, SpentStatus};
        use tracing::{debug, info};

        use super::*;
        use crate::indexer::IndexerParamsWithApi;
        mock! {
            TitanIndexer {}
            impl Clone for TitanIndexer {
                fn clone(&self) -> Self;
            }

            #[async_trait]
            impl TitanApi for TitanIndexer {
                async fn get_status(&self) -> Result<Status, Error>;
                async fn get_tip(&self) -> Result<BlockTip, Error>;
                async fn get_block(&self, query: &query::Block) -> Result<Block, Error>;
                async fn get_block_hash_by_height(&self, height: u64) -> Result<String, Error>;
                async fn get_block_txids(&self, query: &query::Block) -> Result<Vec<String>, Error>;
                async fn get_address(&self, address: &str) -> Result<AddressData, Error>;
                async fn get_transaction(&self, txid: &Txid) -> Result<Transaction, Error>;
                async fn get_transaction_raw(&self, txid: &Txid) -> Result<Vec<u8>, Error>;
                async fn get_transaction_hex(&self, txid: &Txid) -> Result<String, Error>;
                async fn get_transaction_status(&self, txid: &Txid) -> Result<TransactionStatus, Error>;
                async fn send_transaction(&self, tx_hex: String) -> Result<Txid, Error>;
                async fn get_output(&self, outpoint: &OutPoint) -> Result<TxOut, Error>;
                async fn get_inscription(
                    &self,
                    inscription_id: &InscriptionId,
                ) -> Result<(HeaderMap, Vec<u8>), Error>;
                async fn get_runes(
                    &self,
                    pagination: Option<Pagination>,
                ) -> Result<PaginationResponse<RuneResponse>, Error>;
                async fn get_rune(&self, rune: &query::Rune) -> Result<RuneResponse, Error>;
                async fn get_rune_transactions(
                    &self,
                    rune: &query::Rune,
                    pagination: Option<Pagination>,
                ) -> Result<PaginationResponse<Txid>, Error>;
                async fn get_mempool_txids(&self) -> Result<Vec<Txid>, Error>;
                async fn get_mempool_entry(&self, txid: &Txid) -> Result<MempoolEntry, Error>;
                async fn get_mempool_entries(
                    &self,
                    txids: &[Txid],
                ) -> Result<HashMap<Txid, Option<MempoolEntry>>, Error>;
                async fn get_all_mempool_entries(&self) -> Result<HashMap<Txid, MempoolEntry>, Error>;
                async fn get_mempool_entries_with_ancestors(
                    &self,
                    txids: &[Txid],
                ) -> Result<HashMap<Txid, MempoolEntry>, Error>;
                async fn get_subscription(&self, id: &str) -> Result<Subscription, Error>;
                async fn list_subscriptions(&self) -> Result<Vec<Subscription>, Error>;
                async fn add_subscription(&self, subscription: &Subscription) -> Result<Subscription, Error>;
                async fn delete_subscription(&self, id: &str) -> Result<(), Error>;
            }
        }

        #[tokio::test]
        async fn test_retrieving_of_finalized_account_data() -> anyhow::Result<()> {
            const MAX_I_INDEX: u64 = 5;
            const TX_VALUE: u64 = 100;
            const ADDR: &str = "<some_account>";

            dotenv::dotenv()?;
            let _logger_guard = &*TEST_LOGGER;
            let btc_rpc_creds = BtcRpcCredentials::new()?;
            let db_pool = PostgresRepo::from_config(PostgresDbCredentials::new()?)
                .await?
                .into_shared();
            let app_config = ServerConfig::init_config(ConfigVariant::Local)?;

            let generate_utxos = |addr: &str, index: u64| {
                let generate_utxo = || AddressTxOut {
                    txid: Txid::from_str("f74516e3b24af90fc2da8251d2c1e3763252b15c7aec3c1a42dde7116138caee").unwrap(),
                    vout: 0,
                    value: 100,
                    runes: vec![],
                    risky_runes: vec![],
                    spent: SpentStatus::Unspent,
                    status: TransactionStatus::unconfirmed(),
                };
                let mut utxos = vec![];
                // fill unconfirmed
                for j in 0..(MAX_I_INDEX - index) {
                    let mut utxo = generate_utxo();
                    utxo.status = TransactionStatus::unconfirmed();
                    utxos.push(utxo);
                }
                // fill confirmed
                for j in 0..index {
                    let mut utxo = generate_utxo();
                    utxo.status = TransactionStatus::confirmed(j as u64, BlockHash::all_zeros());
                    utxos.push(utxo);
                }
                utxos
            };
            let generate_mocking_invocations = |indexer: &mut MockTitanIndexer| {
                let mut i = 0;
                indexer.expect_get_address().returning(move |addr| {
                    let utxos = generate_utxos(addr, i);
                    i += 1;
                    Ok(AddressData {
                        value: i * TX_VALUE,
                        runes: vec![],
                        outputs: utxos,
                    })
                });
                indexer.expect_clone().returning(move || {
                    let mut cloned_mocked_indexer = MockTitanIndexer::new();
                    let mut i = 0;
                    cloned_mocked_indexer.expect_get_address().returning(move |addr| {
                        let utxos = generate_utxos(addr, i);
                        i += 1;
                        Ok(AddressData {
                            value: i * TX_VALUE,
                            runes: vec![],
                            outputs: utxos,
                        })
                    });
                    cloned_mocked_indexer
                        .expect_clone()
                        .returning(|| MockTitanIndexer::new());
                    cloned_mocked_indexer
                });
            };

            debug!("Initializing mocked indexer");
            let mut mocked_indexer = MockTitanIndexer::new();
            generate_mocking_invocations(&mut mocked_indexer);
            debug!("Building BtcIndexer...");
            let indexer = BtcIndexer::new(IndexerParamsWithApi {
                indexer_params: IndexerParams {
                    btc_rpc_creds,
                    db_pool,
                    btc_indexer_params: app_config.btc_indexer_config,
                },
                titan_api_client: mocked_indexer,
            })?;
            debug!("Tracking account changes..");
            let oneshot = indexer.track_account_changes(ADDR)?;
            debug!("Receiving oneshot event..");
            let result = oneshot.await?;
            debug!("Event: {result:?}");
            assert!(compare_address_tx_outs(
                &result.account_data.outputs,
                &generate_utxos(ADDR, 5)
            ));
            assert_eq!(result.address, ADDR.to_string());
            Ok(())
        }

        #[tokio::test]
        async fn test_retrieving_of_finalized_tx() -> anyhow::Result<()> {
            const MAX_I_INDEX: u64 = 5;

            let tx_id = Txid::from_str("f74516e3b24af90fc2da8251d2c1e3763252b15c7aec3c1a42dde7116138caee")?;

            dotenv::dotenv()?;
            let _logger_guard = &*TEST_LOGGER;
            let btc_rpc_creds = BtcRpcCredentials::new()?;
            let db_pool = PostgresRepo::from_config(PostgresDbCredentials::new()?)
                .await?
                .into_shared();
            let app_config = ServerConfig::init_config(ConfigVariant::Local)?;

            let generate_transaction = |tx_id: &Txid, index: u64| Transaction {
                txid: tx_id.clone(),
                version: 0,
                lock_time: 0,
                input: vec![],
                output: vec![],
                status: if index == MAX_I_INDEX {
                    TransactionStatus::confirmed(index, BlockHash::all_zeros())
                } else {
                    TransactionStatus::unconfirmed()
                },
                size: 0,
                weight: 0,
            };
            let generate_mocking_invocations = |indexer: &mut MockTitanIndexer| {
                let mut i = 0;
                indexer.expect_get_transaction().returning(move |tx_id| {
                    let utxos = generate_transaction(tx_id, i);
                    i += 1;
                    Ok(generate_transaction(tx_id, i))
                });
                indexer.expect_clone().returning(move || {
                    let mut cloned_mocked_indexer = MockTitanIndexer::new();
                    let mut i = 0;
                    cloned_mocked_indexer.expect_get_transaction().returning(move |tx_id| {
                        let utxos = generate_transaction(tx_id, i);
                        i += 1;
                        Ok(generate_transaction(tx_id, i))
                    });
                    cloned_mocked_indexer
                        .expect_clone()
                        .returning(|| MockTitanIndexer::new());
                    cloned_mocked_indexer
                });
            };

            debug!("Initializing mocked indexer");
            let mut mocked_indexer = MockTitanIndexer::new();
            generate_mocking_invocations(&mut mocked_indexer);
            debug!("Building BtcIndexer...");
            let indexer = BtcIndexer::new(IndexerParamsWithApi {
                indexer_params: IndexerParams {
                    btc_rpc_creds,
                    db_pool,
                    btc_indexer_params: app_config.btc_indexer_config,
                },
                titan_api_client: mocked_indexer,
            })?;
            debug!("Tracking tx changes..");
            let oneshot = indexer.track_tx_changes(tx_id)?;
            debug!("Receiving oneshot event..");
            let result = oneshot.await?;
            debug!("Event: {result:?}");
            assert!(compare_address_tx(&result, &generate_transaction(&tx_id, MAX_I_INDEX)));
            Ok(())
        }
    }

    fn compare_address_tx_outs(a: &[AddressTxOut], b: &[AddressTxOut]) -> bool {
        if a.len() != b.len() {
            return false;
        }
        a.iter()
            .zip(b.iter())
            .all(|(x, y)| x.value == y.value && x.status == y.status && x.txid == y.txid && x.vout == y.vout)
    }

    fn compare_address_tx(a: &Transaction, b: &Transaction) -> bool {
        a.status == b.status
            && a.txid == b.txid
            && a.input.iter().zip(b.input.iter()).all(|(x, y)| {
                x.witness == y.witness
                    && x.sequence == y.sequence
                    && x.previous_output == y.previous_output
                    && x.script_sig == y.script_sig
            })
            && a.lock_time == b.lock_time
            && a.output == b.output
            && a.weight == b.weight
            && a.size == b.size
            && a.weight == b.weight
            && a.version == b.version
    }
}
