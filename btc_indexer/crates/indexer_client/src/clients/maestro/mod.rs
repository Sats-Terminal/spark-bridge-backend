pub mod models;

use async_trait::async_trait;
use bitcoin::{Address, Network, OutPoint, Txid, secp256k1::PublicKey};
use btc_indexer_config::MaestroClientConfig;
use lrc20::token_metadata::{
    DEFAULT_IS_FREEZABLE, DEFAULT_TOKEN_TICKER, MAX_SYMBOL_SIZE, MIN_SYMBOL_SIZE, SPARK_CREATION_ENTITY_PUBLIC_KEY,
    TokenMetadata,
};
use ordinals::RuneId;
use reqwest::{Client, Url};
use serde::de::DeserializeOwned;
use std::{collections::HashMap, str::FromStr};
use tracing::{debug, error};

use crate::{
    client_api::{AddrUtxoData, BlockchainInfo, BtcIndexer, OutPointData, RuneData, Runer},
    clients::maestro::models::{
        AddrUtxoMempoolResponse, BlockInfoResponse, MempoolTxInfoResponse, OutputVariant, RuneInfoResponse,
        TxInfoResponse,
    },
    error::BtcIndexerClientError,
};

#[derive(Clone)]
pub struct MaestroClient {
    confirmation_threshold: u64,
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
            confirmation_threshold: config.confirmation_threshold,
        }
    }

    async fn do_get_request<T: DeserializeOwned>(
        &self,
        url: &str,
        query: Option<Vec<(String, String)>>,
    ) -> Result<T, BtcIndexerClientError> {
        let url = self.base_url.join(url)?.to_string();
        debug!(method = "GET", ?url, "performing request");

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

    async fn get_rune_info(&self, rune_id: &str) -> Result<RuneInfoResponse, BtcIndexerClientError> {
        let rune_info_url = format!("assets/runes/{}", rune_id);
        self.do_get_request::<RuneInfoResponse>(&rune_info_url, None).await
    }
}

#[async_trait]
impl BtcIndexer for MaestroClient {
    async fn get_transaction_outpoint(
        &self,
        outpoint: OutPoint,
    ) -> Result<Option<OutPointData>, BtcIndexerClientError> {
        let tx_info_url = format!("mempool/transactions/{}/metaprotocols", outpoint.txid);
        let tx_info = self.do_get_request::<MempoolTxInfoResponse>(&tx_info_url, None).await?;
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
            block_height: self.do_get_request::<u64>("esplora/blocks/tip/height", None).await?,
        })
    }

    async fn get_block_transactions(&self, block_height: u64) -> Result<Vec<Txid>, BtcIndexerClientError> {
        let block_info_url = format!("blocks/{}", block_height);
        let block_info = self.do_get_request::<BlockInfoResponse>(&block_info_url, None).await?;

        let block_txids_url = format!("esplora/block/{}/txids", block_info.data.hash);
        let txids = self.do_get_request::<Vec<String>>(&block_txids_url, None).await?;
        Ok(txids
            .iter()
            .map(|txid| Txid::from_str(txid))
            .collect::<Result<Vec<Txid>, _>>()?)
    }

    async fn get_rune_id(&self, txid: &Txid) -> Result<RuneId, BtcIndexerClientError> {
        let tx_info_url = format!("transactions/{}", txid);
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
        let rune_info_response = self.get_rune_info(&rune_id).await?;

        let rune_id = RuneId::from_str(&rune_info_response.data.id)
            .map_err(|e| BtcIndexerClientError::DecodeError(e.to_string()))?;
        Ok(rune_id)
    }

    async fn get_address_utxos(&self, address: Address) -> Result<Vec<AddrUtxoData>, BtcIndexerClientError> {
        let address_runes_utxos_url = format!("mempool/addresses/{}/utxos", address);
        let mut addr_utxos = Vec::new();

        let mut query: Option<Vec<(String, String)>> = None;
        loop {
            let response = self
                .do_get_request::<AddrUtxoMempoolResponse>(&address_runes_utxos_url, query)
                .await?;

            for addr_utxo in response.data.iter() {
                let confirmed = self.confirmation_threshold == 0
                    || (response
                        .indexer_info
                        .chain_tip
                        .block_height
                        .saturating_sub(addr_utxo.height)
                        >= self.confirmation_threshold);
                addr_utxos.push(AddrUtxoData {
                    spent: false,
                    confirmed,
                    txid: addr_utxo.txid.clone(),
                    vout: addr_utxo.vout,
                    value: addr_utxo.satoshis,
                    runes: addr_utxo
                        .runes
                        .iter()
                        .map(|rune| {
                            let rune_data = RuneData {
                                rune_id: RuneId::from_str(&rune.rune_id)
                                    .map_err(|err| BtcIndexerClientError::DecodeError(err.to_string()))?,
                                amount: rune.amount,
                            };
                            Ok::<_, BtcIndexerClientError>(rune_data)
                        })
                        .collect::<Result<Vec<RuneData>, BtcIndexerClientError>>()?,
                });
            }

            query = match response.next_cursor {
                Some(cursor) => Some(vec![("cursor".to_string(), cursor)]),
                None => {
                    return Ok(addr_utxos);
                }
            };
        }
    }
}

#[async_trait]
impl Runer for MaestroClient {
    async fn get_rune_metadata(
        &self,
        rune_id: &str,
        issuer_public_key: PublicKey,
        network: Network,
    ) -> Result<TokenMetadata, BtcIndexerClientError> {
        let rune_info_response = self.get_rune_info(rune_id).await?;
        let symbol = match rune_info_response.data.symbol {
            Some(symbol) => {
                let width = symbol.len().clamp(MIN_SYMBOL_SIZE, MAX_SYMBOL_SIZE);
                format!("{:<width$}", symbol, width = width)
            }
            None => DEFAULT_TOKEN_TICKER.to_string(),
        };
        Ok(TokenMetadata {
            issuer_public_key,
            network,
            name: rune_id.to_string(),
            symbol,
            decimal: rune_info_response.data.divisibility as u8,
            max_supply: rune_info_response
                .data
                .max_supply
                .parse::<u128>()
                .map_err(|err| BtcIndexerClientError::DecodeError(err.to_string()))?,
            is_freezable: DEFAULT_IS_FREEZABLE,
            creation_entity_public_key: Some(
                PublicKey::from_slice(&SPARK_CREATION_ENTITY_PUBLIC_KEY)
                    .map_err(|err| BtcIndexerClientError::DecodeError(err.to_string()))?,
            ),
        })
    }
}
