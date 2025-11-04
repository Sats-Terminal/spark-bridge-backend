use std::{collections::HashMap, sync::Arc};

use crate::api::BtcIndexerApi;
use crate::tx_arbiter::{TxArbiter, TxArbiterTrait};
use async_trait::async_trait;
use bitcoin::Network;
use bitcoincore_rpc::{RawTx, bitcoin, json};
use btc_indexer_api::api::TrackTxRequest;
use config_parser::config::{BtcIndexerParams, BtcRpcCredentials, MaestroConfig, TitanConfig};

use local_db_store_indexer::init::IndexerDbBounds;
use local_db_store_indexer::init::LocalDbStorage;

use crate::error::BtcIndexerError;
use crate::maestro_client::MaestroClient;
use bitcoin_rpc_client::{BitcoinRpcClient, CoreBitcoinRpcClient, MaestroBitcoinRpcClient};
use titan_client::{TitanApi, TitanClient};
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tracing::instrument;
use uuid::Uuid;

#[derive(Clone)]
pub enum IndexerClient {
    Titan(TitanClient),
    Maestro(MaestroClient),
}

#[async_trait]
impl TitanApi for IndexerClient {
    async fn get_status(&self) -> Result<titan_types::Status, titan_client::Error> {
        match self {
            IndexerClient::Titan(client) => client.get_status().await,
            IndexerClient::Maestro(client) => client.get_status().await,
        }
    }

    async fn get_tip(&self) -> Result<titan_types::BlockTip, titan_client::Error> {
        match self {
            IndexerClient::Titan(client) => client.get_tip().await,
            IndexerClient::Maestro(client) => client.get_tip().await,
        }
    }

    async fn get_block(&self, query: &titan_types::query::Block) -> Result<titan_types::Block, titan_client::Error> {
        match self {
            IndexerClient::Titan(client) => client.get_block(query).await,
            IndexerClient::Maestro(client) => client.get_block(query).await,
        }
    }

    async fn get_block_hash_by_height(&self, height: u64) -> Result<String, titan_client::Error> {
        match self {
            IndexerClient::Titan(client) => client.get_block_hash_by_height(height).await,
            IndexerClient::Maestro(client) => client.get_block_hash_by_height(height).await,
        }
    }

    async fn get_block_txids(&self, query: &titan_types::query::Block) -> Result<Vec<String>, titan_client::Error> {
        match self {
            IndexerClient::Titan(client) => client.get_block_txids(query).await,
            IndexerClient::Maestro(client) => client.get_block_txids(query).await,
        }
    }

    async fn get_address(&self, address: &str) -> Result<titan_types::AddressData, titan_client::Error> {
        match self {
            IndexerClient::Titan(client) => client.get_address(address).await,
            IndexerClient::Maestro(client) => client.get_address(address).await,
        }
    }

    async fn get_transaction(&self, txid: &bitcoin::Txid) -> Result<titan_types::Transaction, titan_client::Error> {
        match self {
            IndexerClient::Titan(client) => client.get_transaction(txid).await,
            IndexerClient::Maestro(client) => client.get_transaction(txid).await,
        }
    }

    async fn get_transaction_raw(&self, txid: &bitcoin::Txid) -> Result<Vec<u8>, titan_client::Error> {
        match self {
            IndexerClient::Titan(client) => client.get_transaction_raw(txid).await,
            IndexerClient::Maestro(client) => client.get_transaction_raw(txid).await,
        }
    }

    async fn get_transaction_hex(&self, txid: &bitcoin::Txid) -> Result<String, titan_client::Error> {
        match self {
            IndexerClient::Titan(client) => client.get_transaction_hex(txid).await,
            IndexerClient::Maestro(client) => client.get_transaction_hex(txid).await,
        }
    }

    async fn get_transaction_status(
        &self,
        txid: &bitcoin::Txid,
    ) -> Result<titan_types::TransactionStatus, titan_client::Error> {
        match self {
            IndexerClient::Titan(client) => client.get_transaction_status(txid).await,
            IndexerClient::Maestro(client) => client.get_transaction_status(txid).await,
        }
    }

    async fn send_transaction(&self, tx_hex: String) -> Result<bitcoin::Txid, titan_client::Error> {
        match self {
            IndexerClient::Titan(client) => client.send_transaction(tx_hex).await,
            IndexerClient::Maestro(client) => client.send_transaction(tx_hex).await,
        }
    }

    async fn get_output(&self, outpoint: &bitcoin::OutPoint) -> Result<titan_types::TxOut, titan_client::Error> {
        match self {
            IndexerClient::Titan(client) => client.get_output(outpoint).await,
            IndexerClient::Maestro(client) => client.get_output(outpoint).await,
        }
    }

    async fn get_inscription(
        &self,
        inscription_id: &titan_types::InscriptionId,
    ) -> Result<(reqwest::header::HeaderMap, Vec<u8>), titan_client::Error> {
        match self {
            IndexerClient::Titan(client) => client.get_inscription(inscription_id).await,
            IndexerClient::Maestro(client) => client.get_inscription(inscription_id).await,
        }
    }

    async fn get_runes(
        &self,
        pagination: Option<titan_types::Pagination>,
    ) -> Result<titan_types::PaginationResponse<titan_types::RuneResponse>, titan_client::Error> {
        match self {
            IndexerClient::Titan(client) => client.get_runes(pagination).await,
            IndexerClient::Maestro(client) => client.get_runes(pagination).await,
        }
    }

    async fn get_rune(
        &self,
        rune: &titan_types::query::Rune,
    ) -> Result<titan_types::RuneResponse, titan_client::Error> {
        match self {
            IndexerClient::Titan(client) => client.get_rune(rune).await,
            IndexerClient::Maestro(client) => client.get_rune(rune).await,
        }
    }

    async fn get_rune_transactions(
        &self,
        rune: &titan_types::query::Rune,
        pagination: Option<titan_types::Pagination>,
    ) -> Result<titan_types::PaginationResponse<bitcoin::Txid>, titan_client::Error> {
        match self {
            IndexerClient::Titan(client) => client.get_rune_transactions(rune, pagination).await,
            IndexerClient::Maestro(client) => client.get_rune_transactions(rune, pagination).await,
        }
    }

    async fn get_mempool_txids(&self) -> Result<Vec<bitcoin::Txid>, titan_client::Error> {
        match self {
            IndexerClient::Titan(client) => client.get_mempool_txids().await,
            IndexerClient::Maestro(client) => client.get_mempool_txids().await,
        }
    }

    async fn get_mempool_entry(&self, txid: &bitcoin::Txid) -> Result<titan_types::MempoolEntry, titan_client::Error> {
        match self {
            IndexerClient::Titan(client) => client.get_mempool_entry(txid).await,
            IndexerClient::Maestro(client) => client.get_mempool_entry(txid).await,
        }
    }

    async fn get_mempool_entries(
        &self,
        txids: &[bitcoin::Txid],
    ) -> Result<HashMap<bitcoin::Txid, Option<titan_types::MempoolEntry>>, titan_client::Error> {
        match self {
            IndexerClient::Titan(client) => client.get_mempool_entries(txids).await,
            IndexerClient::Maestro(client) => client.get_mempool_entries(txids).await,
        }
    }

    async fn get_all_mempool_entries(
        &self,
    ) -> Result<HashMap<bitcoin::Txid, titan_types::MempoolEntry>, titan_client::Error> {
        match self {
            IndexerClient::Titan(client) => client.get_all_mempool_entries().await,
            IndexerClient::Maestro(client) => client.get_all_mempool_entries().await,
        }
    }

    async fn get_mempool_entries_with_ancestors(
        &self,
        txids: &[bitcoin::Txid],
    ) -> Result<HashMap<bitcoin::Txid, titan_types::MempoolEntry>, titan_client::Error> {
        match self {
            IndexerClient::Titan(client) => client.get_mempool_entries_with_ancestors(txids).await,
            IndexerClient::Maestro(client) => client.get_mempool_entries_with_ancestors(txids).await,
        }
    }

    async fn get_subscription(&self, id: &str) -> Result<titan_types::Subscription, titan_client::Error> {
        match self {
            IndexerClient::Titan(client) => client.get_subscription(id).await,
            IndexerClient::Maestro(client) => client.get_subscription(id).await,
        }
    }

    async fn list_subscriptions(&self) -> Result<Vec<titan_types::Subscription>, titan_client::Error> {
        match self {
            IndexerClient::Titan(client) => client.list_subscriptions().await,
            IndexerClient::Maestro(client) => client.list_subscriptions().await,
        }
    }

    async fn add_subscription(
        &self,
        subscription: &titan_types::Subscription,
    ) -> Result<titan_types::Subscription, titan_client::Error> {
        match self {
            IndexerClient::Titan(client) => client.add_subscription(subscription).await,
            IndexerClient::Maestro(client) => client.add_subscription(subscription).await,
        }
    }

    async fn delete_subscription(&self, id: &str) -> Result<(), titan_client::Error> {
        match self {
            IndexerClient::Titan(client) => client.delete_subscription(id).await,
            IndexerClient::Maestro(client) => client.delete_subscription(id).await,
        }
    }
}

pub struct BtcIndexer<C, Db, TxValidator> {
    pub btc_indexer_params: BtcIndexerParams,
    persistent_storage: Db,
    indexer_client: Arc<C>,
    tx_validator: Arc<TxValidator>,
    btc_core: Arc<dyn BitcoinRpcClient>,
    cancellation_token: CancellationToken,
    task_tracker: TaskTracker,
}

pub struct IndexerParamsWithApi<C, Db, TxValidator> {
    pub indexer_params: IndexerParams<Db>,
    pub indexer_client: Arc<C>,
    pub tx_validator: Arc<TxValidator>,
    pub bitcoin_rpc_client: Arc<dyn BitcoinRpcClient>,
}

pub struct IndexerParams<Db> {
    pub titan_config: Option<TitanConfig>,
    pub maestro_config: Option<MaestroConfig>,
    pub btc_rpc_creds: BtcRpcCredentials,
    pub db_pool: Db,
    pub btc_indexer_params: BtcIndexerParams,
}

impl BtcIndexer<IndexerClient, LocalDbStorage, TxArbiter> {
    pub fn with_api(mut params: IndexerParams<LocalDbStorage>) -> crate::error::Result<Self> {
        let network = params.btc_rpc_creds.network;
        let (indexer_client, bitcoin_rpc_client): (IndexerClient, Arc<dyn BitcoinRpcClient>) = match network {
            Network::Regtest => {
                let titan_config = params.titan_config.take().ok_or(BtcIndexerError::MissingTitanConfig)?;
                let core_client =
                    CoreBitcoinRpcClient::new(params.btc_rpc_creds.url.as_str(), params.btc_rpc_creds.get_btc_creds())?;
                let rpc: Arc<dyn BitcoinRpcClient> = Arc::new(core_client);
                (IndexerClient::Titan(TitanClient::new(titan_config.url.as_ref())), rpc)
            }
            _ => {
                let maestro_config = params
                    .maestro_config
                    .take()
                    .ok_or(BtcIndexerError::MissingMaestroConfig)?;
                let maestro_rpc =
                    MaestroBitcoinRpcClient::new(maestro_config.base_url.clone(), maestro_config.api_key.clone())?;
                let rpc: Arc<dyn BitcoinRpcClient> = Arc::new(maestro_rpc);
                (
                    IndexerClient::Maestro(MaestroClient::new(&maestro_config, network)?),
                    rpc,
                )
            }
        };

        Self::new(IndexerParamsWithApi {
            indexer_params: params,
            indexer_client: Arc::new(indexer_client),
            tx_validator: Arc::new(TxArbiter {}),
            bitcoin_rpc_client,
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
            params.indexer_client.clone(),
            params.tx_validator.clone(),
            &mut task_tracker,
        );
        tracing::debug!(
            "Initialization passed with configuration, {:?}",
            params.indexer_params.btc_indexer_params
        );
        let indexer = BtcIndexer {
            btc_indexer_params: params.indexer_params.btc_indexer_params,
            persistent_storage: params.indexer_params.db_pool,
            indexer_client: params.indexer_client,
            tx_validator: params.tx_validator,
            btc_core: params.bitcoin_rpc_client,
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
        let _ = self
            .indexer_client
            .get_status()
            .await
            .map_err(|e| BtcIndexerError::HealthcheckError(format!("Unable to retrieve indexer status, err: {e}")));
        Ok(())
    }

    #[instrument(level = "trace", skip(self))]
    fn get_tx_info(&self, tx_id: bitcoin::Txid) -> crate::error::Result<bitcoin::transaction::Transaction> {
        Ok(self.btc_core.get_transaction(&tx_id)?)
    }

    #[instrument(level = "trace", skip(self))]
    fn get_blockchain_info(&self) -> crate::error::Result<json::GetBlockchainInfoResult> {
        Ok(self.btc_core.get_blockchain_info()?)
    }

    #[instrument(level = "trace", skip(self, tx))]
    fn broadcast_transaction(&self, tx: impl RawTx) -> crate::error::Result<bitcoin::blockdata::transaction::Txid> {
        let hex_tx = tx.raw_hex();
        tracing::info!("broadcasting transaction, {hex_tx}");
        Ok(self.btc_core.send_raw_transaction(&hex_tx)?)
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
