use crate::error::GatewayError;
use crate::init::AppState;
use axum::Json;
use axum::extract::{Path, State};
use bitcoin::secp256k1::PublicKey;
use gateway_local_db_store::schemas::deposit_address::{DepositActivity, DepositAddrInfo, DepositStatus};
use gateway_local_db_store::schemas::utxo_storage::UtxoStatus;
use serde::Serialize;
use serde_json::Value;
use std::collections::{HashMap, hash_map::Entry};
use std::env;
use std::str::FromStr;
use tracing::{instrument, warn};
use url::Url;

#[instrument(level = "trace", skip(state), ret)]
pub async fn handle(
    State(state): State<AppState>,
    Path(user_public_key): Path<String>,
) -> Result<Json<Vec<UserBridgeActivity>>, GatewayError> {
    let public_key = PublicKey::from_str(&user_public_key)
        .map_err(|e| GatewayError::InvalidData(format!("Invalid public key: {e}")))?;

    let activity = state
        .deposit_verification_aggregator
        .list_user_activity(public_key)
        .await
        .map_err(|e| GatewayError::DepositVerificationError(e.to_string()))?;

    let metadata_entries = state
        .deposit_verification_aggregator
        .list_wrune_metadata()
        .await
        .map_err(|e| GatewayError::DepositVerificationError(e.to_string()))?;

    let metadata_map: HashMap<String, Value> = metadata_entries
        .into_iter()
        .map(|entry| (entry.rune_id, entry.wrune_metadata))
        .collect();

    let maestro_client = MaestroClient::new();
    let response = build_activity_response(activity, &metadata_map, maestro_client.as_ref()).await;

    Ok(Json(response))
}

#[instrument(level = "trace", skip(state), ret)]
pub async fn handle_transaction(
    State(state): State<AppState>,
    Path(txid): Path<String>,
) -> Result<Json<UserBridgeActivity>, GatewayError> {
    let activity = state
        .deposit_verification_aggregator
        .get_activity_by_txid(&txid)
        .await
        .map_err(|e| GatewayError::DepositVerificationError(e.to_string()))?;

    let Some(record) = activity else {
        return Err(GatewayError::InvalidData(format!("Transaction {txid} not found")));
    };

    let metadata_entries = state
        .deposit_verification_aggregator
        .list_wrune_metadata()
        .await
        .map_err(|e| GatewayError::DepositVerificationError(e.to_string()))?;

    let metadata_map: HashMap<String, Value> = metadata_entries
        .into_iter()
        .map(|entry| (entry.rune_id, entry.wrune_metadata))
        .collect();

    let maestro_client = MaestroClient::new();
    let mut response = build_activity_response(vec![record], &metadata_map, maestro_client.as_ref()).await;

    if let Some(activity) = response.pop() {
        Ok(Json(activity))
    } else {
        Err(GatewayError::InvalidData(format!("Transaction {txid} not found")))
    }
}

#[derive(Debug, Serialize)]
pub struct UserBridgeActivity {
    pub rune_id: String,
    pub amount: u64,
    pub normalized_amount: u64,
    pub btc_deposit_address: String,
    pub spark_bridge_address: Option<String>,
    pub status: String,
    pub confirmations: Option<u64>,
    pub txid: Option<String>,
    pub vout: Option<u32>,
    pub wrune_metadata: Option<Value>,
}

fn parse_out_point(out_point: &str) -> Option<(String, u32)> {
    let mut parts = out_point.split(':');
    let txid = parts.next()?.to_string();
    let vout_str = parts.next()?;
    let vout = vout_str.parse::<u32>().ok()?;
    Some((txid, vout))
}

fn overall_status(info: &DepositAddrInfo, utxo_status: Option<UtxoStatus>) -> String {
    if info
        .confirmation_status
        .responses
        .values()
        .any(|status| *status == DepositStatus::Failed)
    {
        return "failed".to_string();
    }

    match utxo_status {
        Some(UtxoStatus::Spent) => "spent".to_string(),
        Some(UtxoStatus::Confirmed) => "minted".to_string(),
        _ => {
            if info.confirmation_status.check_all_verifiers_confirmed() {
                "ready_for_mint".to_string()
            } else if info
                .confirmation_status
                .responses
                .values()
                .any(|status| *status == DepositStatus::WaitingForConfirmation)
            {
                "waiting_for_confirmations".to_string()
            } else if info
                .confirmation_status
                .responses
                .values()
                .all(|status| *status == DepositStatus::Created)
            {
                "address_issued".to_string()
            } else {
                "pending".to_string()
            }
        }
    }
}

struct MaestroClient {
    client: reqwest::Client,
    base_url: Url,
}

impl MaestroClient {
    fn new() -> Option<Self> {
        let base = env::var("MAESTRO_API_URL").ok()?;
        let api_key = env::var("MAESTRO_API_KEY").ok()?;

        let mut default_headers = reqwest::header::HeaderMap::new();
        let api_key_value = reqwest::header::HeaderValue::from_str(&api_key).ok()?;
        default_headers.insert("api-key", api_key_value);

        let client = reqwest::Client::builder()
            .default_headers(default_headers)
            .build()
            .ok()?;

        let mut base_url = Url::parse(&base).ok()?;
        let needs_v0 = {
            let path = base_url.path();
            !(path.ends_with("/v0/") || path.ends_with("/v0"))
        };
        {
            let mut segments = base_url.path_segments_mut().ok()?;
            segments.pop_if_empty();
            if needs_v0 {
                segments.push("v0");
            }
            segments.push("");
        }

        Some(Self { client, base_url })
    }

    async fn confirmations(&self, txid: &str) -> Result<Option<u64>, reqwest::Error> {
        let request_url = format!("{}transactions/{txid}", self.base_url);
        let response = self.client.get(request_url).send().await?;
        if !response.status().is_success() {
            return Ok(None);
        }
        let envelope = response.json::<MaestroEnvelope<MaestroTransactionData>>().await?;
        Ok(envelope.data.confirmations)
    }

    async fn rune_divisibility(&self, rune_id: &str) -> Result<Option<u8>, reqwest::Error> {
        let request_url = format!("{}assets/runes/{rune_id}", self.base_url);
        let response = self.client.get(request_url).send().await?;
        if !response.status().is_success() {
            return Ok(None);
        }
        let envelope = response.json::<MaestroEnvelope<MaestroRuneData>>().await?;
        Ok(envelope.data.divisibility)
    }
}

#[derive(serde::Deserialize)]
struct MaestroEnvelope<T> {
    data: T,
}

#[derive(serde::Deserialize)]
struct MaestroTransactionData {
    confirmations: Option<u64>,
}

#[derive(serde::Deserialize)]
struct MaestroRuneData {
    divisibility: Option<u8>,
}

async fn build_activity_response(
    activity: Vec<DepositActivity>,
    metadata_map: &HashMap<String, Value>,
    maestro_client: Option<&MaestroClient>,
) -> Vec<UserBridgeActivity> {
    let mut confirmations_cache: HashMap<String, Option<u64>> = HashMap::new();
    let mut divisibility_cache: HashMap<String, Option<u8>> = HashMap::new();
    let mut response = Vec::with_capacity(activity.len());

    for record in activity.into_iter() {
        let info = &record.deposit_info;
        let rune_id = record.rune_id.clone();
        let status = overall_status(info, record.utxo_status);

        let (txid, vout) = match record
            .out_point
            .as_ref()
            .and_then(|out_point| parse_out_point(out_point))
        {
            Some((txid, vout)) => (Some(txid), Some(vout)),
            None => (None, None),
        };

        let needs_confirmations = matches!(status.as_str(), "waiting_for_confirmations" | "pending");
        let confirmations = match (needs_confirmations, txid.as_ref(), maestro_client) {
            (true, Some(tx), Some(client)) => match confirmations_cache.entry(tx.clone()) {
                Entry::Occupied(entry) => *entry.get(),
                Entry::Vacant(entry) => {
                    let fetched = match client.confirmations(tx).await {
                        Ok(value) => value,
                        Err(err) => {
                            warn!("Failed to fetch confirmations for {tx}: {err}");
                            None
                        }
                    };
                    entry.insert(fetched);
                    fetched
                }
            },
            _ => None,
        };

        let wrune_metadata_value = metadata_map.get(&rune_id);
        let metadata_decimals = wrune_metadata_value
            .and_then(|meta| meta.get("decimals"))
            .and_then(|val| val.as_u64())
            .map(|val| val as u8);

        let divisibility = if let Some(dec) = metadata_decimals {
            Some(dec)
        } else if let Some(client) = maestro_client {
            match divisibility_cache.entry(rune_id.clone()) {
                Entry::Occupied(entry) => entry.get().clone(),
                Entry::Vacant(entry) => {
                    let fetched = match client.rune_divisibility(&rune_id).await {
                        Ok(value) => value,
                        Err(err) => {
                            warn!("Failed to fetch rune metadata for {}: {}", rune_id, err);
                            None
                        }
                    };
                    entry.insert(fetched);
                    fetched
                }
            }
        } else {
            None
        };

        let amount_display = derive_display_amount(info, wrune_metadata_value, divisibility);
        let wrune_metadata = wrune_metadata_value.cloned();

        response.push(UserBridgeActivity {
            rune_id,
            amount: amount_display,
            normalized_amount: info.amount,
            btc_deposit_address: info.deposit_address.to_string(),
            spark_bridge_address: info.bridge_address.as_ref().map(|addr| addr.to_string()),
            status,
            confirmations,
            txid,
            vout,
            wrune_metadata,
        });
    }

    response
}

fn derive_display_amount(info: &DepositAddrInfo, wrune_metadata: Option<&Value>, divisibility_hint: Option<u8>) -> u64 {
    if info.requested_amount != info.amount {
        return info.requested_amount;
    }

    let decimals = wrune_metadata
        .and_then(|meta| meta.get("decimals"))
        .and_then(|val| val.as_u64())
        .map(|val| val as u8)
        .or(divisibility_hint)
        .unwrap_or(0)
        .min(18) as u32;

    if decimals == 0 {
        return info.amount;
    }

    let factor = 10u128.checked_pow(decimals).unwrap_or(1);
    (info.amount as u128 / factor) as u64
}
