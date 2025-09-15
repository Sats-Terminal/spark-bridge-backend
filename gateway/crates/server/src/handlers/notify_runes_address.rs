use crate::error::GatewayError;
use crate::init::AppState;
use axum::Json;
use axum::extract::State;
use btc_resp_aggregator::aggregator::BtcConfirmationsAggregator;
use btc_resp_aggregator::traits::CheckTxRequest;
use global_utils::common_types::{TxIdWrapped, get_uuid};
use global_utils::network::convert_to_http_url;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use tracing::instrument;
use uuid::Uuid;

#[derive(Deserialize, Debug)]
pub struct NotifyRunesAddressRequest {
    pub user_public_key: String,
    pub tx: TxIdWrapped,
    pub spark_address: String,
}

#[derive(Serialize, Debug)]
pub struct NotifyRunesAddressResponse {
    pub uuid_to_track_bridge: Uuid,
}

#[instrument(level = "info", skip(request, state), fields(request = ?request), ret)]
pub async fn handle(
    State(state): State<AppState>,
    Json(request): Json<NotifyRunesAddressRequest>,
) -> Result<Json<NotifyRunesAddressResponse>, GatewayError> {
    let user_pubkey = bitcoin::secp256k1::PublicKey::from_str(&request.user_public_key)?;
    let uuid = get_uuid();
    state
        .btc_resp_checker
        .send_tx_to_verifiers(CheckTxRequest {
            uuid,
            tx_id: request.tx.0,
            public_key: user_pubkey,
            loopback_addr: convert_to_http_url(
                state.private_api_socket_addr.clone(),
                Some(BtcConfirmationsAggregator::LOOPBACK_ENDPOINT_PATH),
            )?,
        })
        .await?;
    Ok(Json(NotifyRunesAddressResponse {
        uuid_to_track_bridge: uuid,
    }))
}
