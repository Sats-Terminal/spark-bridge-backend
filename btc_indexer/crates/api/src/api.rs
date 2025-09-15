use axum::Json;
use global_utils::api_result_request::{ApiResponseOwned, Empty};
use global_utils::common_types::{TxIdWrapped, UrlWrapped};
use serde::{Deserialize, Serialize};
use titan_client::Transaction;
use utoipa::ToSchema;

pub struct BtcIndexerApi;

impl BtcIndexerApi {
    pub const TRACK_TX_ENDPOINT: &'static str = "https://api.trc";
    pub const TRACK_WALLET_ENDPOINT: &'static str = "https://api.trc";
}

#[derive(Deserialize, Serialize, ToSchema, Debug)]
#[schema(example = json!({
    "tx_id": "fb0c9ab881331ec7acdd85d79e3197dcaf3f95055af1703aeee87e0d853e81ec",
    "callback_url": "http://127.0.0.1:8080"
}))]
pub struct TrackTxRequest {
    pub tx_id: TxIdWrapped,
    pub callback_url: UrlWrapped,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
#[schema(example = json!({
    "wallet": "sprt1pgss8fxt9jxuv4dgjwrg539s6u06ueausq076xvfej7wdah0htvjlxunt9fa4n",
    "callback_url": "127.0.0.1:8080"
}))]
pub struct TrackWalletRequest {
    pub wallet_id: String,
    pub callback_url: UrlWrapped,
}

pub type TrackTxResponse = Json<Empty>;
pub type TrackWalletResponse = Json<Empty>;
pub type BtcIndexerCallbackResponse = ApiResponseOwned<Transaction>;
