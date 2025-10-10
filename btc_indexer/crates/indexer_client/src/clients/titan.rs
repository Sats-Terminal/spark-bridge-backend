use titan_client::TitanClient as TitanInnerClient;
use titan_client::TitanApi;
use crate::client_api::{BtcIndexerClientApi, OutPointData, BlockchainInfo};
use crate::error::BtcIndexerClientError;
use bitcoin::OutPoint;
use async_trait::async_trait;
use std::collections::HashMap;
use ordinals::RuneId;
use std::str::FromStr;
use btc_indexer_config::IndexerClientConfig;

pub struct TitanClient {
    client: TitanInnerClient,
}

#[async_trait]
impl BtcIndexerClientApi for TitanClient {
    fn new(config: IndexerClientConfig) -> Self {
        Self { client: TitanInnerClient::new(&config.url) }
    }

    async fn get_transaction_outpoint(&self, outpoint: OutPoint) -> Result<Option<OutPointData>, BtcIndexerClientError> {
        let response = self.client.get_transaction(&outpoint.txid).await;
        let response = match response {
            Ok(response) => response,
            Err(e) => {
                tracing::warn!("Failed to get transaction outpoint: {:?}", e);
                return Ok(None);
            }
        };
        let block_height = response.status.block_height.ok_or(BtcIndexerClientError::InvalidData("Block height not found".to_string()))?;
        let output = response.output.get(outpoint.vout as usize).ok_or(BtcIndexerClientError::VoutOutOfRange(outpoint.vout, response.output.len() as u32))?;
        let mut runes = HashMap::new();
        for rune in output.runes.iter() {
            let rune_id = RuneId::from_str(&rune.rune_id.to_string()).map_err(|e| BtcIndexerClientError::DecodeError(format!("Failed to parse rune id: {}", e)))?;
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
        Ok(BlockchainInfo { block_height: response.block_tip.height })
    }
}
