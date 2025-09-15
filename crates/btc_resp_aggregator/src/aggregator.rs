use crate::error::BtcAggregatorError;
use crate::traits::{CheckTxRequest, CheckTxResponse, TxCheckerClientTrait};
use bitcoin::Txid;
use frost_secp256k1_tr::Identifier;
use gateway_api::api::{BridgeRunesToSparkRequest, Review, TxCheckCallbackResponse};
use global_utils::api_result_request::ApiResponseOwned;
use global_utils::common_types::TxIdWrapped;
use reqwest::Client;
use std::collections::{BTreeMap, HashMap, HashSet, LinkedList};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::interval;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, instrument, trace, warn};
use url::Url;
use uuid::Uuid;

const BTC_AGGREGATOR_LOG_PATH: &str = "btc-aggregator";

pub type BtcVerifiers = BTreeMap<Identifier, Arc<dyn TxCheckerClientTrait>>;
type CachedStorage = Arc<RwLock<HashMap<Txid, VerificationState>>>;
type FinishedTx = (Txid, VerificationState);

#[derive(Default, Debug, Clone)]
pub struct VerificationState {
    against: HashSet<u16>,
    approve: HashSet<u16>,
    tx_id: Option<Txid>,
}

/// Spawns under the hood task that would update and send information to `gateway` once at specified interval
pub struct BtcConfirmationsAggregator {
    verifiers: BtcVerifiers, // TODO: implement btc verifiers
    //todo: change tx_id on uuid
    cached_storage: CachedStorage,
    cancellation_token: CancellationToken,
}

impl VerificationState {
    pub fn is_confirmed(&self, threshold: usize) -> bool {
        self.approve.len() > threshold
    }
}

impl Drop for BtcConfirmationsAggregator {
    fn drop(&mut self) {
        self.cancellation_token.cancel();
    }
}
pub struct BtcAggregatorParams {
    pub threshold: u16,
    pub total_participants: u16,
    pub interval_millisecond: u64,
    /// Url in which Btc Aggregator on finished task would send information about negotiated tx
    pub bridge_runes_gateway_url: Url,
}

impl BtcConfirmationsAggregator {
    /// Enpoint path is used as a loopback address in private `gateway` router
    pub const LOOPBACK_ENDPOINT_PATH: &'static str = "/loopback_indexer_response";
    /// Endpoint path means that it will only receive confirmed requests which has to be only
    ///  executed using local saved info
    pub const RUN_BRIDGE_RUNE_SPARK_FLOW_PATH: &'static str = "/api/user/bridge-runes";

    pub fn new(verifiers: BtcVerifiers, params: BtcAggregatorParams) -> Self {
        let cached_storage: CachedStorage = Arc::new(RwLock::new(HashMap::default()));
        let cancellation_token = CancellationToken::new();
        Self::spawn_updating_task(cached_storage.clone(), params, cancellation_token.child_token());
        Self {
            verifiers,
            cached_storage,
            cancellation_token,
        }
    }

    fn spawn_updating_task(
        cached_storage: CachedStorage,
        params: BtcAggregatorParams,
        cancellation_token: CancellationToken,
    ) {
        tokio::spawn({
            let BtcAggregatorParams {
                threshold,
                total_participants,
                interval_millisecond,
                bridge_runes_gateway_url: gateway_url,
            } = params;
            let client = Client::new();
            let mut interval = interval(Duration::from_millis(interval_millisecond));
            async move {
                'checking_loop: loop {
                    tokio::select! {
                        //todo: add removing of unconfirmed txs (maybe refactor it)
                        _ = interval.tick() => {
                            let finished_txs = Self::check_storage(
                                cached_storage.clone(),
                                threshold as usize)
                            .await;
                            let _ = Self::notify_gateway(&client, gateway_url.clone(), finished_txs)
                                .await
                                .inspect_err(|e|
                                    error!("[{BTC_AGGREGATOR_LOG_PATH}] [Updating task] Failed to notify gateway, url: {gateway_url}, reason: {e:?}"));
                        }
                         _ = cancellation_token.cancelled() => {
                            debug!("[{BTC_AGGREGATOR_LOG_PATH}] Closing [Updating task] in btc confirmation aggregator, because of cancellation token");
                            break 'checking_loop;
                        },
                    };
                }
            }
        });
    }

    /// Checks and removes outdated `tx_id`s to check
    async fn check_storage(cached_storage: CachedStorage, threshold: usize) -> LinkedList<FinishedTx> {
        let mut lock = cached_storage.write().await;
        let mut keys_to_remove = LinkedList::new();
        for (k, v) in lock.iter() {
            if v.is_confirmed(threshold) {
                keys_to_remove.push_back((k.clone(), v.clone()));
            }
        }
        for (uuid, _) in keys_to_remove.iter() {
            lock.remove(uuid);
        }
        keys_to_remove
    }

    async fn notify_gateway(
        client: &Client,
        notify_url: Url,
        finished_txs: LinkedList<FinishedTx>,
    ) -> Result<(), reqwest::Error> {
        for (uuid, state) in finished_txs {
            if let Some(tx_id) = state.tx_id {
                client
                    .post(notify_url.clone())
                    .json(&BridgeRunesToSparkRequest {
                        //todo: insert valid uuid
                        uuid: Uuid::default(),
                        tx: TxIdWrapped(tx_id),
                    })
                    .send()
                    .await?;
            }
        }
        Ok(())
    }

    pub async fn send_tx_to_verifiers(&self, req: CheckTxRequest) -> Result<(), BtcAggregatorError> {
        {
            let mut lock = self.cached_storage.write().await;
            lock.insert(req.tx_id, VerificationState::default());
        }

        let mut jobs_to_send = Vec::with_capacity(self.verifiers.len());
        for (_id, verifier) in self.verifiers.iter() {
            jobs_to_send.push(verifier.check_tx(req.clone()));
        }
        let res: Result<Vec<CheckTxResponse>, BtcAggregatorError> =
            futures::future::join_all(jobs_to_send).await.into_iter().collect();
        let res = res?;
        for x in res {
            if let ApiResponseOwned::Err { code, message } = x.response {
                return Err(BtcAggregatorError::FailedToSendMsgToVerifier { code, message });
            }
        }
        Ok(())
    }

    #[instrument(skip(self), level = "trace")]
    pub async fn save_verifier_response(&self, request: TxCheckCallbackResponse) -> Result<(), BtcAggregatorError> {
        let mut lock = self.cached_storage.write().await;
        match lock.get_mut(&request.tx.txid) {
            None => {
                trace!(
                    verifier_id = request.identifier,
                    "Sent overdue response to tx_id: {}", request.tx.txid
                )
            }
            Some(state) => match request.review_description {
                Review::Accept => {
                    state.approve.insert(request.identifier);
                    trace!(verifier_id = request.identifier, tx_id =? request.tx.txid, "Verifier approve");
                }
                Review::Rejected { description } => {
                    state.against.insert(request.identifier);
                    warn!(verifier_id = request.identifier, tx_id =? request.tx.txid, "Verifier is against tx_id, description: '{description}'");
                }
            },
        }
        Ok(())
    }
}
