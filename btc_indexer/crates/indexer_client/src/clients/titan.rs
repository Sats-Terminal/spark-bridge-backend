use async_trait::async_trait;
use bitcoin::{OutPoint, Txid, hashes::Hash};
use btc_indexer_config::TitanClientConfig;
use ordinals::RuneId;
use std::{collections::HashMap, str::FromStr};
use titan_client::{TitanApi, TitanClient as TitanInnerClient, query::Block};
use tracing::warn;

use crate::{
    client_api::{BlockchainInfo, BtcIndexerClientApi, OutPointData},
    error::BtcIndexerClientError,
};

#[derive(Clone)]
pub struct TitanClient {
    client: TitanInnerClient,
}

impl TitanClient {
    pub fn new(config: &TitanClientConfig) -> Self {
        Self {
            client: TitanInnerClient::new(&config.url),
        }
    }
}

#[async_trait]
impl BtcIndexerClientApi for TitanClient {
    async fn get_transaction_outpoint(
        &self,
        outpoint: OutPoint,
    ) -> Result<Option<OutPointData>, BtcIndexerClientError> {
        let response = self.client.get_transaction(&outpoint.txid).await;
        let response = match response {
            Ok(response) => response,
            Err(e) => {
                warn!("Failed to get transaction outpoint: {:?}", e);
                return Ok(None);
            }
        };
        let block_height = response
            .status
            .block_height
            .ok_or(BtcIndexerClientError::InvalidData("Block height not found".to_string()))?;
        let output = response
            .output
            .get(outpoint.vout as usize)
            .ok_or(BtcIndexerClientError::VoutOutOfRange(
                outpoint.vout,
                response.output.len() as u32,
            ))?;
        let mut runes = HashMap::new();
        for rune in output.runes.iter() {
            let rune_id = RuneId::from_str(&rune.rune_id.to_string())
                .map_err(|e| BtcIndexerClientError::DecodeError(format!("Failed to parse rune id: {}", e)))?;
            runes.insert(rune_id, rune.amount);
        }

        Ok(Some(OutPointData {
            outpoint,
            block_height,
            rune_amounts: runes,
            sats_amount: output.value,
        }))
    }

    async fn get_blockchain_info(&self) -> Result<BlockchainInfo, BtcIndexerClientError> {
        let response = self.client.get_status().await?;
        Ok(BlockchainInfo {
            block_height: response.block_tip.height,
        })
    }

    async fn get_block_transactions(&self, block_height: u64) -> Result<Vec<Txid>, BtcIndexerClientError> {
        let response = self.client.get_block(&Block::Height(block_height)).await?;
        let txids = response
            .tx_ids
            .iter()
            .map(|txid| Txid::from_byte_array(txid.0))
            .collect();
        Ok(txids)
    }
}
