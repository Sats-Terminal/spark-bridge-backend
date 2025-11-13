pub mod models;

use async_trait::async_trait;
use bitcoin::{Address, Network, OutPoint, Txid, secp256k1::PublicKey};
use btc_indexer_config::MaestroClientConfig;
use lrc20::token_metadata::{
    DEFAULT_IS_FREEZABLE, DEFAULT_TOKEN_TICKER, MAX_SYMBOL_SIZE, MIN_SYMBOL_SIZE, SPARK_CREATION_ENTITY_PUBLIC_KEY,
    TokenMetadata,
};
use ordinals::RuneId;
use reqwest::{Client, StatusCode, Url};
use serde::de::DeserializeOwned;
use std::{collections::HashMap, convert::TryFrom, str::FromStr};
use tracing::{debug, error};

use crate::{
    client_api::{AddrUtxoData, BlockchainInfo, BtcIndexer, OutPointData, RuneData, Runer},
    clients::maestro::models::{
        AddrUtxoMempoolResponse, BlockInfoResponse, MempoolTxInfoResponse, OutputMetaprotocols, RuneInfoResponse,
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
        Ok(self.do_get_request::<RuneInfoResponse>(&rune_info_url, None).await?)
    }
}

#[async_trait]
impl BtcIndexer for MaestroClient {
    async fn get_transaction_outpoint(
        &self,
        outpoint: OutPoint,
    ) -> Result<Option<OutPointData>, BtcIndexerClientError> {
        if let Some(tx_info) = self.fetch_mempool_transaction(&outpoint).await? {
            return Ok(Some(Self::build_outpoint_data(
                outpoint,
                &tx_info.data.outputs,
                tx_info.data.height,
            )?));
        }

        let tx_info_url = format!("transactions/{}/metaprotocols", outpoint.txid.to_string());
        let tx_info = self.do_get_request::<TxInfoResponse>(&tx_info_url, None).await?;

        Ok(Some(Self::build_outpoint_data(
            outpoint,
            &tx_info.data.outputs,
            tx_info.data.height,
        )?))
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
            .map(|txid| Txid::from_str(&txid))
            .collect::<Result<Vec<Txid>, _>>()?)
    }

    async fn get_rune_id(&self, txid: &Txid) -> Result<RuneId, BtcIndexerClientError> {
        let tx_info_url = format!("transactions/{}", txid.to_string());
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
        let address_runes_utxos_url = format!("mempool/addresses/{}/utxos", address.to_string());
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
                    confirmed: confirmed,
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
                                amount: parse_rune_amount_to_u64(&rune.amount)?,
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

impl MaestroClient {
    async fn fetch_mempool_transaction(
        &self,
        outpoint: &OutPoint,
    ) -> Result<Option<MempoolTxInfoResponse>, BtcIndexerClientError> {
        let tx_info_url = format!("mempool/transactions/{}/metaprotocols", outpoint.txid.to_string());
        let url = self.base_url.join(&tx_info_url)?;

        let request = self.api_client.get(url.clone()).header("api-key", &self.api_key);
        let response = self.api_client.execute(request.build()?).await?;

        match response.status() {
            StatusCode::NOT_FOUND => Ok(None),
            status if status.is_success() => {
                let body = response.text().await?;
                debug!(method = "GET", url = url.as_str(), body = body, "performing request");
                let parsed = serde_json::from_str::<MempoolTxInfoResponse>(&body).map_err(|err| {
                    error!(?err, "Err happened during parsing");
                    BtcIndexerClientError::InvalidData(format!("Failed to do request: {}", url))
                })?;
                Ok(Some(parsed))
            }
            status => {
                let body = response.text().await.unwrap_or_else(|_| "N/A".to_string());
                error!(
                    url = url.as_str(),
                    status = status.as_str(),
                    body,
                    "Failed to do request"
                );
                Err(BtcIndexerClientError::InvalidData(format!(
                    "Failed to do request: {}",
                    url
                )))
            }
        }
    }

    fn build_outpoint_data(
        outpoint: OutPoint,
        outputs: &[OutputMetaprotocols],
        height: u64,
    ) -> Result<OutPointData, BtcIndexerClientError> {
        let output = Self::extract_output(outputs, outpoint.vout)?;

        let mut runes = HashMap::new();
        for rune in output.runes.iter() {
            let rune_id = RuneId::from_str(&rune.rune_id.to_string())
                .map_err(|e| BtcIndexerClientError::DecodeError(format!("Failed to parse rune id: {}", e)))?;
            let rune_amount = parse_rune_amount_to_u128(&rune.amount)?;
            runes.insert(rune_id, rune_amount);
        }

        Ok(OutPointData {
            outpoint,
            block_height: height,
            rune_amounts: runes,
            sats_amount: output.base.satoshis,
        })
    }

    fn extract_output<'a>(
        outputs: &'a [OutputMetaprotocols],
        vout: u32,
    ) -> Result<&'a OutputMetaprotocols, BtcIndexerClientError> {
        let output = outputs
            .get(vout as usize)
            .ok_or(BtcIndexerClientError::VoutOutOfRange(vout, outputs.len() as u32))?;

        Ok(output)
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
        let rune_info_response = self.get_rune_info(&rune_id).await?;
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

fn parse_rune_amount_to_u128(amount: &str) -> Result<u128, BtcIndexerClientError> {
    if amount.trim().is_empty() {
        return Err(BtcIndexerClientError::DecodeError(
            "Empty rune amount received from Maestro".to_string(),
        ));
    }

    let mut digits = String::with_capacity(amount.len());
    for ch in amount.chars() {
        match ch {
            '0'..='9' => digits.push(ch),
            '.' => continue,
            _ => {
                return Err(BtcIndexerClientError::DecodeError(format!(
                    "Unsupported rune amount format: {}",
                    amount
                )));
            }
        }
    }

    if digits.is_empty() {
        return Err(BtcIndexerClientError::DecodeError(format!(
            "Failed to parse rune amount: {}",
            amount
        )));
    }

    digits
        .parse::<u128>()
        .map_err(|err| BtcIndexerClientError::DecodeError(format!("Failed to parse rune amount '{}': {}", amount, err)))
}

fn parse_rune_amount_to_u64(amount: &str) -> Result<u64, BtcIndexerClientError> {
    let value = parse_rune_amount_to_u128(amount)?;
    u64::try_from(value)
        .map_err(|_| BtcIndexerClientError::DecodeError(format!("Rune amount '{}' exceeds supported range", amount)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clients::maestro::models::MempoolTxInfoResponse;

    #[test]
    fn parse_rune_amount_handles_decimals() {
        let samples = [
            ("100.00000", 10_000_000u128),
            ("10.00", 1_000u128),
            ("5.64", 564u128),
            ("2010.95191", 201_095_191u128),
            ("1", 1u128),
        ];

        for (input, expected) in samples {
            assert_eq!(parse_rune_amount_to_u128(input).unwrap(), expected);
        }
    }

    #[test]
    fn maestro_mempool_output_with_runes_deserializes() {
        let sample = r#"{
            "data": {
                "fees": "600",
                "height": 923345,
                "inputs": [],
                "metaprotocols": ["runes"],
                "outputs": [
                    {
                        "address": null,
                        "script_pubkey": "6a5d0b160200caa2338b07e80701",
                        "satoshis": "0",
                        "spending_tx": null,
                        "inscriptions": [],
                        "runes": []
                    },
                    {
                        "address": "bc1ps7v39ewrg2rmgp7d5tnkfrhwjzaz7sle9d25r6m8ym4zpxhec6aqljkk0s",
                        "script_pubkey": "5120879912e5c34287b407cda2e7648eee90ba2f43f92b5541eb6726ea209af9c6ba",
                        "satoshis": "546",
                        "spending_tx": null,
                        "inscriptions": [],
                        "runes": [
                            { "rune_id": "840010:907", "amount": "10.00" }
                        ]
                    }
                ],
                "sats_per_vb": 2,
                "volume": "36000"
            },
            "indexer_info": {
                "chain_tip": {
                    "block_hash": "00000000000000000001459d59abb04744ef289a9345435ab09c2b50389a46ab",
                    "block_height": 923446
                },
                "mempool_timestamp": "2025-11-13 12:30:09",
                "estimated_blocks": [
                    {
                        "block_height": 923447,
                        "sats_per_vb": { "min": 1, "median": 3, "max": 301 }
                    }
                ]
            }
        }"#;

        let parsed: MempoolTxInfoResponse = serde_json::from_str(sample).unwrap();
        let out = parsed.data.outputs.get(1).expect("output exists");
        assert_eq!(out.runes.len(), 1);
        assert_eq!(out.runes[0].amount, "10.00");
    }
}
