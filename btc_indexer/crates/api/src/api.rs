use axum::Json;
use bitcoin::{OutPoint, Txid};
use global_utils::common_resp::Empty;
use global_utils::common_types::{Url, UrlWrapped};
use ordinals::RuneId;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::str::FromStr;
use utoipa::{PartialSchema, openapi};

pub struct BtcIndexerApi;

impl BtcIndexerApi {
    /// Represents hardcoded `/track_tx` endpoint
    pub const TRACK_TX_ENDPOINT: &'static str = "/track_tx";
    pub const HEALTHCHECK_ENDPOINT: &'static str = "/healthcheck";
}

pub type Amount = u64;
pub type VOut = u32;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct TrackTxRequest {
    pub btc_address: String,
    pub out_point: OutPoint,
    pub rune_id: RuneId,
    pub rune_amount: Amount,
    pub callback_url: UrlWrapped,
}

impl PartialSchema for TrackTxRequest {
    fn schema() -> openapi::RefOr<openapi::schema::Schema> {
        utoipa::openapi::ObjectBuilder::new()
            .schema_type(utoipa::openapi::schema::SchemaType::Type(openapi::schema::Type::String))
            .examples(Some(json!(&TrackTxRequest {
                callback_url: UrlWrapped(Url::parse("https://api.trc.btcindex.net").unwrap()),
                btc_address: "bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh".to_string(),
                out_point: OutPoint {
                    txid: Txid::from_str("fb0c9ab881331ec7acdd85d79e3197dcaf3f95055af1703aeee87e0d853e81ec",).unwrap(),
                    vout: 32
                },
                rune_id: RuneId::from_str("840000:3").unwrap(),
                rune_amount: 100,
            })))
            .into()
    }
}
impl utoipa::ToSchema for TrackTxRequest {
    fn name() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed("TrackTxRequest")
    }
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
    pub outpoint: OutPoint,
    pub status: BtcTxReview,
    pub sats_fee_amount: Amount,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub enum BtcTxReview {
    Success,
    Failure { reason: TxRejectReason },
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub enum TxRejectReason {
    NoRunesInOuts,
    NoFeesPayed,
    TooFewSatoshiPaidAsFee { got: u64, at_least_expected: u64 },
    NoExpectedVOutInOutputs { got: u64, expected: u64 },
    NoExpectedTOutWithRunes,
    NoExpectedTOutWithRunesAmount { amount: u64 },
}
