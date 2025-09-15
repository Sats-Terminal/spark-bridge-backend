use crate::error::BtcTxCheckerError;
use crate::traits::{BtcTxIdStatusStorage, CheckTxRequest, TxIdStatusValue, TxidStatus};
use bitcoin::Txid;
use btc_indexer_api::api::{BtcIndexerCallbackResponse, TrackTxRequest};
use gateway_api::api::{Review, TxCheckCallbackResponse};
use global_utils::api_result_request::ApiResponseOwned;
use global_utils::common_types::{TxIdWrapped, UrlWrapped};
use global_utils::network::convert_to_http_url;
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, ToSocketAddrs};
use std::sync::Arc;
use titan_client::Transaction;
use url::Url;

const INDEXER_SUBSCRIBE_TX_ENDPOINT: &str = "/track_tx";
pub type BtcTxChekerIdentifier = u16;

#[derive(Clone)]
pub struct BtcTxChecker {
    pub identifier: BtcTxChekerIdentifier,
    tx_id_status_storage: Arc<dyn BtcTxIdStatusStorage>, // TODO: implement signer storage
    total_participants: u16,
    threshold: u16,
    /// Address of verifier where BtcTxChecker has to work
    pub gateway_url: Url,
    pub btc_indexer_url: Url,
    http_client: reqwest::Client,
}

impl BtcTxChecker {
    pub const LOOPBACK_ENDPOINT_PATH: &'static str = "/api/verifier/receive/loopback_btc_indexer_response";
    pub fn new(
        identifier: BtcTxChekerIdentifier,
        total_participants: u16,
        threshold: u16,
        gateway_addr: (IpAddr, u16),
        btc_indexer_url: Url,
        tx_id_status_storage: Arc<dyn BtcTxIdStatusStorage>,
    ) -> Result<Self, BtcTxCheckerError> {
        Ok(Self {
            identifier,
            tx_id_status_storage,
            total_participants,
            gateway_url: convert_to_http_url(gateway_addr, Some(Self::LOOPBACK_ENDPOINT_PATH))
                .map_err(|e| BtcTxCheckerError::UrlParseError(e))?,
            threshold,
            btc_indexer_url,
            http_client: reqwest::Client::new(),
        })
    }

    pub async fn save_tx(&self, request: &CheckTxRequest) -> Result<(), BtcTxCheckerError> {
        self.tx_id_status_storage
            .set_tx_id_value(
                request.tx_id,
                &TxIdStatusValue {
                    gateway_loopback_addr: UrlWrapped(request.loopback_addr.clone()),
                    status: TxidStatus::Created,
                },
            )
            .await?;
        Ok(())
    }

    pub async fn subscribe_indexer_to_loopback_addr(&self, tx_id: Txid) -> Result<(), BtcTxCheckerError> {
        self.tx_id_status_storage
            .set_tx_id_status(tx_id, &TxidStatus::Created)
            .await?;
        let _ = self
            .http_client
            .post(format!("{}{INDEXER_SUBSCRIBE_TX_ENDPOINT}", self.btc_indexer_url))
            .json(&TrackTxRequest {
                tx_id: TxIdWrapped(tx_id),
                callback_url: UrlWrapped(self.btc_indexer_url.clone()),
            })
            .send()
            .await?;
        Ok(())
    }

    #[inline]
    fn check_tx(&self, tx_to_check: Transaction) -> TxCheckCallbackResponse {
        TxCheckCallbackResponse {
            identifier: self.identifier,
            review_description: Review::Accept,
            tx: tx_to_check,
        }
    }

    pub async fn notify_gateway(&self, request: BtcIndexerCallbackResponse) -> Result<(), BtcTxCheckerError> {
        match request {
            BtcIndexerCallbackResponse::Ok { data } => {
                let check_tx_response = self.check_tx(data);
                self.http_client
                    .post(self.gateway_url.clone())
                    .json(&check_tx_response)
                    .send()
                    .await?;
                Ok(())
            }
            BtcIndexerCallbackResponse::Err { code, message } => {
                Err(BtcTxCheckerError::IndexerResponseError { code, message })
            }
        }
    }
}
