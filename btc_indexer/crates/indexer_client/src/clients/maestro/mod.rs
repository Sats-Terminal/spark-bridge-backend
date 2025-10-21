pub mod models;

use async_trait::async_trait;
use bitcoin::{OutPoint, Txid};
use btc_indexer_config::MaestroClientConfig;
use ordinals::RuneId;
use reqwest::Client;
use serde::de::DeserializeOwned;
use std::{collections::HashMap, str::FromStr};
use tracing::{debug, error};

use crate::{
    client_api::{BlockchainInfo, BtcIndexer, OutPointData},
    clients::maestro::models::{BlockInfoResponse, TxInfoMetaprotocolsResponse},
    error::BtcIndexerClientError,
};

#[derive(Clone)]
pub struct MaestroClient {
    api_key: String,
    base_url: String,
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

    async fn do_get_request<T: DeserializeOwned>(&self, url: &str) -> Result<T, BtcIndexerClientError> {
        let url = format!("{}/{}", self.base_url, url);
        let request = self.api_client.get(&url).header("api-key", &self.api_key).build()?;
        let response = self.api_client.execute(request).await?;

        if response.status().is_success() {
            let txt = response.text().await?;
            debug!(?txt, "Resp body");
            let body = match serde_json::from_str::<T>(&txt) {
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
            // return Ok(response.json::<T>().await?);
            // return Err(BtcIndexerClientError::InvalidData(format!(
            // "Failed to do request: {}",
            // url
            // )));
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
        let tx_info = self.do_get_request::<TxInfoMetaprotocolsResponse>(&tx_info_url).await?;
        let output = tx_info
            .data
            .outputs
            .get(outpoint.vout as usize)
            .ok_or(BtcIndexerClientError::VoutOutOfRange(
                outpoint.vout,
                tx_info.data.outputs.len() as u32,
            ))?;
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
            sats_amount: output.satoshis,
        }))
    }

    async fn get_blockchain_info(&self) -> Result<BlockchainInfo, BtcIndexerClientError> {
        Ok(BlockchainInfo {
            block_height: self.do_get_request::<u64>("/esplora/blocks/tip/height").await?,
        })
    }

    async fn get_block_transactions(&self, block_height: u64) -> Result<Vec<Txid>, BtcIndexerClientError> {
        let block_info_url = format!("blocks/{}", block_height);
        let block_info = self.do_get_request::<BlockInfoResponse>(&block_info_url).await?;

        let block_txids_url = format!("/esplora/block/{}/txids", block_info.data.hash);
        let txids = self.do_get_request::<Vec<String>>(&block_txids_url).await?;
        Ok(txids
            .iter()
            .map(|txid| Txid::from_str(&txid))
            .collect::<Result<Vec<Txid>, _>>()?)
    }
}
