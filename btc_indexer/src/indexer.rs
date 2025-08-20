use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use bitcoincore_rpc::{bitcoin, json, Client, RpcApi};
use config_parser::config::BtcRpcCredentials;
use titan_client::TitanTcpClient;
use tokio::sync::{mpsc::Sender, RwLock};
use tokio_util::sync::CancellationToken;
use tracing::instrument;
use persistent_storage::init::PersistentRepoShared;
use crate::api::{BtcIndexerApi, Subscription, SubscriptionEvents};

#[derive()]
pub struct BtcIndexer {
    //todo: maybe move into traits?
    subscription_storage: Arc<RwLock<HashMap<Subscription, Sender<SubscriptionEvents>>>>,
    indexer_client: TitanTcpClient,
    btc_core: Arc<Client>,
    cancellation_token: CancellationToken,
}

pub struct IndexerParams {
    btc_rpc_creds: BtcRpcCredentials,
    db_pool: Arc<PersistentRepoShared>,
}

impl BtcIndexer {
    pub fn new(params: IndexerParams) -> crate::error::Result<Self> {
        let storage = Arc::new(RwLock::new(HashMap::new()));
        let cancellation_token = CancellationToken::new();
        let titan_client = Arc::new(TitanTcpClient::new());

        let btc_rpc_client = Arc::new(Client::new(
            &params.btc_rpc_creds.url.to_string(),
            params.btc_rpc_creds.get_btc_creds(),
        )?);
        Self::open_listener(
            storage.clone(),
            titan_client.clone(),

            btc_rpc_client.clone(),
            cancellation_token.child_token(),
        );
        Ok(BtcIndexer {
            subscription_storage: storage,
            indexer_client: TitanTcpClient::new(),
            btc_core: btc_rpc_client,
            cancellation_token,
        })
    }

    #[instrument(skip_all, level = "trace")]
    async fn open_listener(
        storage: Arc<RwLock<HashMap<Subscription, Sender<SubscriptionEvents>>>>,
        titan_client: Arc<TitanTcpClient>,
        rest_client: Arc<Client>,
        cancellation_token: CancellationToken,
    ) {
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = cancellation_token.cancelled() => {
                        info!("Closing HotTokenManager updating task, because of cancellation token");
                        break;
                    },
                    _ = interval.tick() => {
                        let _ = Self::fetch_and_update(&rest_client, &cached_tokens, &url_to_poll)
                            .await
                            .inspect_err(|e| error!("Failed to fetch hot tokens: {:?}", e));
                    }
                }
            }
        });
    }
}

impl Drop for BtcIndexer {
    fn drop(&mut self) {
        self.cancellation_token.cancel()
    }
}

#[async_trait]
impl BtcIndexerApi for BtcIndexer {
    async fn subscribe(options: Subscription) -> crate::error::Result<SubscriptionEvents> {
        todo!()
    }

    fn get_tx_info(&self, tx_id: bitcoin::Txid) -> crate::error::Result<bitcoin::transaction::Transaction> {
        Ok(self.btc_core.get_by_id(&tx_id)?)
    }

    fn get_blockchain_info(&self) -> crate::error::Result<json::GetBlockchainInfoResult> {
        Ok(self.btc_core.get_blockchain_info()?)
    }
}

#[cfg(test)]
mod testing {
    use std::{str::FromStr, time::SystemTime};

    use bitcoincore_rpc::{bitcoin::Txid, RawTx};
    use config_parser::config::BtcRpcCredentials;
    use ordinals::Runestone;
    use titan_client::{EventType, TcpSubscriptionRequest, TitanApi, TitanClient};

    use crate::{
        api::BtcIndexerApi,
        indexer::{BtcIndexer, IndexerParams},
    };

    #[tokio::test]
    async fn init_btc_indexer() -> anyhow::Result<()> {
        dotenv::dotenv()?;
        let btc_rpc_creds = BtcRpcCredentials::new()?;
        let indexer = BtcIndexer::new(IndexerParams { btc_rpc_creds })?;
        println!("Blockchain info: {:?}", indexer.get_blockchain_info()?);
        Ok(())
    }

    #[tokio::test]
    async fn get_btc_tx_by_id() -> anyhow::Result<()> {
        dotenv::dotenv()?;
        let btc_rpc_creds = BtcRpcCredentials::new()?;
        let indexer = BtcIndexer::new(IndexerParams { btc_rpc_creds })?;
        let tx_info = indexer.get_tx_info(Txid::from_str(
            "250f0473c42878dbe9153100100c9c9a55ea85eea688fd358d975351b33d2741",
        )?)?;
        println!("Blockchain info: {:?}", tx_info);
        println!("Blockchain info: {:?}", tx_info.raw_hex());
        println!("Blockchain info: {:?}", tx_info.tx_out(1)?.script_pubkey.as_script());
        let hex = "020704a7d987f890dd81b7f4ebe7d07b0101052406000ae80708904e";
        let bytes = hex::decode(hex)?; // Converts hex string to Vec<u8>
        let etching = Runestone::decipher(&tx_info);
        println!("Parsed ordinals: {:?}", etching);
        // println!("Parsed ordinals: {:?}",ordinals::Etching::deserialize("020704a7d987f890dd81b7f4ebe7d07b0101052406000ae80708904e")?);

        Ok(())
    }

    #[tokio::test]
    async fn it_works() -> anyhow::Result<()> {
        titan_client::TitanTcpClientConfig {
            max_retries: None,
            retry_delay: Default::default(),
            read_buffer_capacity: 0,
            max_buffer_size: 0,
            ping_interval: Default::default(),
            pong_timeout: Default::default(),
        };
        // titan_client::TitanTcpClient::new_with_config(Confi)
        let api = TitanClient::new("http://127.0.0.1:3030");
        println!("AsyncClient status: {:?}", api.get_status().await);
        let btc_rpc_creds = BtcRpcCredentials::new()?;
        let indexer = BtcIndexer::new(IndexerParams { btc_rpc_creds })?;
        // indexer.indexer_client.
        let events = vec![
            EventType::RuneEtched,
            EventType::RuneBurned,
            EventType::RuneMinted,
            EventType::RuneTransferred,
            EventType::AddressModified,
            EventType::TransactionSubmitted,
            EventType::TransactionsAdded,
            EventType::TransactionsReplaced,
            EventType::MempoolTransactionsAdded,
            EventType::MempoolTransactionsReplaced,
            EventType::MempoolEntriesUpdated,
            EventType::NewBlock,
            EventType::Reorg,
        ];
        let mut x = indexer
            .indexer_client
            .subscribe(
                "bc1qepx55kfsgavty5jxa5vyayztcvvk20wkn25ytsahh0g4t7jgekpqe4qh04",
                TcpSubscriptionRequest {
                    subscribe: events.clone(),
                },
            )
            .await?;
        let mut x_2 = indexer
            .indexer_client
            .subscribe(
                "bc1p6fx5ksk4hrnqyesve2ps6w6a2y7j8utlsq54v9r6yn7tc3e5dfvqkgchsk",
                TcpSubscriptionRequest {
                    subscribe: events.clone(),
                },
            )
            .await?;
        let mut x_3 = indexer
            .indexer_client
            .subscribe(
                "bc1q7x0jt49e999ydrwrdwdqmh8nhmh7sf7xf9m8um",
                TcpSubscriptionRequest {
                    subscribe: events.clone(),
                },
            )
            .await?;
        let mut x_4 = indexer
            .indexer_client
            .subscribe(
                "bc1qwzrryqr3ja8w7hnja2spmkgfdcgvqwp5swz4af4ngsjecfz0w0pqud7k38",
                TcpSubscriptionRequest {
                    subscribe: events.clone(),
                },
            )
            .await?;
        let mut x_5 = indexer
            .indexer_client
            .subscribe(
                "bc1qczm7ud0rsku03qx7rtzhkrgvawc3qjcx8dv65v",
                TcpSubscriptionRequest {
                    subscribe: events.clone(),
                },
            )
            .await?;
        println!("{:?}", indexer.indexer_client.get_status());
        loop {
            println!(
                "time: {:?}, status: {:?}",
                SystemTime::now(),
                indexer.indexer_client.get_status()
            );
            tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
            match x.try_recv() {
                Ok(m) => {
                    println!("message: {:?}, time: {:?}", m, SystemTime::now(),);
                }
                Err(e) => {
                    println!("errror: {e:?}, time; {:?}", SystemTime::now(),)
                }
            }
            match x_2.try_recv() {
                Ok(m) => {
                    println!("message2: {:?}, time: {:?}", m, SystemTime::now(),);
                }
                Err(e) => {
                    println!("errror2: {e:?}, time; {:?}", SystemTime::now(),)
                }
            }
            match x_3.try_recv() {
                Ok(m) => {
                    println!("message2: {:?}, time: {:?}", m, SystemTime::now(),);
                }
                Err(e) => {
                    println!("errror2: {e:?}, time; {:?}", SystemTime::now(),)
                }
            }
            match x_4.try_recv() {
                Ok(m) => {
                    println!("message2: {:?}, time: {:?}", m, SystemTime::now(),);
                }
                Err(e) => {
                    println!("errror2: {e:?}, time; {:?}", SystemTime::now(),)
                }
            }
            match x_5.try_recv() {
                Ok(m) => {
                    println!("message2: {:?}, time: {:?}", m, SystemTime::now(),);
                }
                Err(e) => {
                    println!("errror2: {e:?}, time; {:?}", SystemTime::now(),)
                }
            }
        }
        Ok(())
    }
}

// what I have todo:
// * index transaction -> by subscription track transaction and return msg on completion
// * save received transactions in db
// * implement on subscription of some tx, 1) check whether value is in db 2) if true return else subscribe on this event
// * implement on looking in bitcoin d some info
// * get full tx info on demand
//
// * implement rest api entrypoint for starting bridging funds from one place to another
//  + subscribe on replenishment of address
//  +
//
// store in db:
// tx_id, inputs, outputs, parsed tx in runes, raw_tx
