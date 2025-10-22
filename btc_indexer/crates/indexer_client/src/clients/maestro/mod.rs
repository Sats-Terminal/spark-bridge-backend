pub mod models;

use async_trait::async_trait;
use bitcoin::{Address, OutPoint, Txid};
use btc_indexer_config::MaestroClientConfig;
use ordinals::RuneId;
use reqwest::{Client, Url};
use serde::de::DeserializeOwned;
use std::{collections::HashMap, str::FromStr};
use tracing::{debug, error};

use crate::{
    client_api::{BlockchainInfo, BtcIndexer, OutPointData, RuneData, RuneUtxo},
    clients::maestro::models::{BlockInfoResponse, OutputVariant, RuneInfoResponse, RuneUtxoResponse, TxInfoResponse},
    error::BtcIndexerClientError,
};

#[derive(Clone)]
pub struct MaestroClient {
    api_key: String,
    base_url: Url,
    api_client: Client,
}

impl MaestroClient {
    pub fn new(config: &MaestroClientConfig) -> Self {
        Self {
            api_key: config.key.clone(),
            base_url: config.url.clone(),
            api_client: Client::new(),
        }
    }

    async fn do_get_request<T: DeserializeOwned>(
        &self,
        url: &str,
        query: Option<Vec<(&str, &str)>>,
    ) -> Result<T, BtcIndexerClientError> {
        let url = self.base_url.join(url)?.to_string();
        let mut request = self.api_client.get(&url).header("api-key", &self.api_key);
        if let Some(query) = query {
            for (key, value) in query {
                request = request.query(&[(key, value)]);
            }
        }

        let request = request.build()?;
        let response = self.api_client.execute(request).await?;

        if response.status().is_success() {
            let body_str = response.text().await?;
            debug!(body=?body_str, "Resp body");
            let body = match serde_json::from_str::<T>(&body_str) {
                Ok(parsed) => parsed,
                Err(err) => {
                    error!(?err, "Err happened during parsing");
                    return Err(BtcIndexerClientError::InvalidData(format!(
                        "Failed to do request: {}",
                        url
                    )));
                }
            };
            return Ok(body);
        }

        let status = response.status();
        let body = response.text().await.unwrap_or_else(|_| "N/A".to_string());

        error!(url, status = status.as_str(), body, "Failed to do request");

        Err(BtcIndexerClientError::InvalidData(format!(
            "Failed to do request: {}",
            url
        )))
    }
}

#[async_trait]
impl BtcIndexer for MaestroClient {
    async fn get_transaction_outpoint(
        &self,
        outpoint: OutPoint,
    ) -> Result<Option<OutPointData>, BtcIndexerClientError> {
        let tx_info_url = format!("/transactions/{}/metaprotocols", outpoint.txid.to_string());
        let tx_info = self.do_get_request::<TxInfoResponse>(&tx_info_url, None).await?;
        let output = tx_info
            .data
            .outputs
            .get(outpoint.vout as usize)
            .ok_or(BtcIndexerClientError::VoutOutOfRange(
                outpoint.vout,
                tx_info.data.outputs.len() as u32,
            ))?;
        let output = match output {
            OutputVariant::WithMetaprotocols(out) => out,
            _ => return Err(BtcIndexerClientError::DecodeError("Invalid output variant".to_string())),
        };
        let mut runes = HashMap::new();
        for rune in output.runes.iter() {
            let rune_id = RuneId::from_str(&rune.rune_id.to_string())
                .map_err(|e| BtcIndexerClientError::DecodeError(format!("Failed to parse rune id: {}", e)))?;
            runes.insert(rune_id, rune.amount as u128);
        }

        Ok(Some(OutPointData {
            outpoint,
            block_height: tx_info.data.height,
            rune_amounts: runes,
            sats_amount: output.base.satoshis,
        }))
    }

    async fn get_blockchain_info(&self) -> Result<BlockchainInfo, BtcIndexerClientError> {
        Ok(BlockchainInfo {
            block_height: self.do_get_request::<u64>("/esplora/blocks/tip/height", None).await?,
        })
    }

    async fn get_block_transactions(&self, block_height: u64) -> Result<Vec<Txid>, BtcIndexerClientError> {
        let block_info_url = format!("/blocks/{}", block_height);
        let block_info = self.do_get_request::<BlockInfoResponse>(&block_info_url, None).await?;

        let block_txids_url = format!("/esplora/block/{}/txids", block_info.data.hash);
        let txids = self.do_get_request::<Vec<String>>(&block_txids_url, None).await?;
        Ok(txids
            .iter()
            .map(|txid| Txid::from_str(&txid))
            .collect::<Result<Vec<Txid>, _>>()?)
    }

    async fn get_rune_id(&self, txid: &Txid) -> Result<RuneId, BtcIndexerClientError> {
        let tx_info_url = format!("/transactions/{}", txid.to_string());
        let tx_info = self.do_get_request::<TxInfoResponse>(&tx_info_url, None).await?;

        let rune_id = RuneId::new(tx_info.data.height, tx_info.data.tx_index as u32).ok_or(
            BtcIndexerClientError::DecodeError(format!(
                "Failed to build rune id, {}, {}",
                tx_info.data.height, tx_info.data.tx_index
            )),
        )?;
        Ok(rune_id)
    }

    async fn get_rune(&self, rune_id: String) -> Result<RuneId, BtcIndexerClientError> {
        let rune_info_url = format!("/assets/runes/{}", rune_id);
        let rune_info_response = self.do_get_request::<RuneInfoResponse>(&rune_info_url, None).await?;

        let rune_id = RuneId::from_str(&rune_info_response.data.id)
            .map_err(|e| BtcIndexerClientError::DecodeError(e.to_string()))?;
        Ok(rune_id)
    }

    async fn get_address_rune_utxos(&self, address: Address) -> Result<Vec<RuneUtxo>, BtcIndexerClientError> {
        let address_runes_utxos_url = format!("/addresses/{}/runes/utxos", address.to_string());
        let mut rune_utxos = Vec::new();

        let mut cursor = "".to_string();
        loop {
            let response = self
                // TODO: test it on testnet
                .do_get_request::<RuneUtxoResponse>(&address_runes_utxos_url, Some(vec![("cursor", &cursor)]))
                .await?;

            for rune_utxo in response.data.iter() {
                rune_utxos.push(RuneUtxo {
                    spent: false,
                    // TODO: make confirmation configurable
                    confirmed: rune_utxo.confirmations >= 6,
                    txid: rune_utxo.txid.clone(),
                    vout: rune_utxo.vout,
                    value: rune_utxo.satoshis,
                    runes: rune_utxo
                        .runes
                        .iter()
                        .map(|rune| RuneData {
                            // TODO: unwrap ?
                            rune_id: RuneId::from_str(&rune.rune_id).unwrap(),
                            amount: rune.amount,
                        })
                        .collect(),
                });
            }

            cursor = response.next_cursor.clone().unwrap_or_default();
            if response.next_cursor.is_none() {
                break;
            }
        }

        Ok(rune_utxos)
    }
}
