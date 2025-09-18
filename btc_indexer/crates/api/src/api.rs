use axum::Json;
use bitcoin::OutPoint;
use global_utils::common_resp::Empty;
use global_utils::common_types::{TxIdWrapped, UrlWrapped};
use serde::{Deserialize, Serialize};
use sqlx::Type;
use titan_client::{Transaction, TxOut};
use utoipa::ToSchema;

pub struct BtcIndexerApi;

impl BtcIndexerApi {
    pub const TRACK_TX_ENDPOINT: &'static str = "https://api.trc";
    pub const TRACK_WALLET_ENDPOINT: &'static str = "https://api.trc";
}

pub type Amount = u128;
pub type VOut = u32;

#[derive(Deserialize, Serialize, ToSchema, Debug, Clone)]
pub struct OutPointSerialized {
    pub tx_id: TxIdWrapped,
    pub v_out: u32,
}

impl From<OutPointSerialized> for OutPoint {
    fn from(value: OutPointSerialized) -> Self {
        OutPoint {
            txid: value.tx_id.0,
            vout: value.v_out,
        }
    }
}

#[derive(Deserialize, Serialize, ToSchema, Debug, Clone)]
#[schema(example = json!({
    "tx_id": "fb0c9ab881331ec7acdd85d79e3197dcaf3f95055af1703aeee87e0d853e81ec",
    "callback_url": "http://127.0.0.1:8080"
}))]
pub struct TrackTxRequest {
    pub callback_url: UrlWrapped,
    pub btc_address: String,
    pub out_point: OutPointSerialized,
    pub amount: Amount,
}

pub type TrackTxResponse = Json<Empty>;
pub type BtcIndexerCallbackResponse = IndexerCallbackResponse;

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub enum IndexerCallbackResponse {
    Ok {
        meta: ResponseMeta,
    },
    Err {
        code: u16,
        msg: String,
        req_meta: TrackTxRequest,
    },
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ResponseMeta {
    outpoint: OutPoint,
    status: BtcTxReview,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub enum BtcTxReview {
    Success,
    Failure { reason: TxRejectReason },
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub enum TxRejectReason {
    Hello,
}
