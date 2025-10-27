use async_trait::async_trait;
use bitcoin::{Address, OutPoint, Txid, hashes::Hash};
use btc_indexer_config::TitanClientConfig;
use ordinals::RuneId;
use std::{collections::HashMap, str::FromStr};
use titan_client::{
    SpentStatus, TitanApi, TitanClient as TitanInnerClient,
    query::{Block, Rune},
};
use tracing::warn;

use crate::{
    client_api::{AddrUtxoData, BlockchainInfo, BtcIndexer, OutPointData, RuneData},
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
impl BtcIndexer for TitanClient {
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

    async fn get_rune_id(&self, txid: &Txid) -> Result<RuneId, BtcIndexerClientError> {
        let response = self.client.get_transaction(txid).await?;
        let block_height = response
            .status
            .block_height
            .ok_or(BtcIndexerClientError::DecodeError("Block height not found".to_string()))?;
        let block = self.client.get_block(&Block::Height(block_height)).await?;
        let tx_index = block
            .tx_ids
            .iter()
            .position(|id| id.to_string() == txid.to_string())
            .ok_or(BtcIndexerClientError::DecodeError(
                "Transaction not found in block".to_string(),
            ))?;
        let rune_id = RuneId::new(block_height, tx_index as u32).ok_or(BtcIndexerClientError::DecodeError(format!(
            "Failed to build rune id, {}, {}",
            block_height, tx_index
        )))?;
        Ok(rune_id)
    }

    async fn get_rune(&self, rune_id: String) -> Result<RuneId, BtcIndexerClientError> {
        let query_rune = Rune::from_str(&rune_id).map_err(|e| BtcIndexerClientError::DecodeError(e.to_string()))?;
        let rune_response = self.client.get_rune(&query_rune).await?;
        // Build rune id from response to prevent type mismatch err
        let rune_id = RuneId::new(rune_response.id.block, rune_response.id.tx).ok_or(
            BtcIndexerClientError::DecodeError(format!(
                "Failed to build rune id, {}, {}",
                rune_response.id.block, rune_response.id.tx
            )),
        )?;
        Ok(rune_id)
    }

    async fn get_address_utxos(&self, address: Address) -> Result<Vec<AddrUtxoData>, BtcIndexerClientError> {
        let address_data = self.client.get_address(&address.to_string()).await?;
        let mut rune_utxos = Vec::new();

        for output in address_data.outputs.iter() {
            rune_utxos.push(AddrUtxoData {
                spent: !matches!(output.spent, SpentStatus::Unspent),
                confirmed: output.status.confirmed,
                txid: output.txid.to_string(),
                vout: output.vout,
                value: output.value,
                runes: output
                    .runes
                    .iter()
                    .map(|rune| RuneData {
                        rune_id: RuneId {
                            block: rune.rune_id.block,
                            tx: rune.rune_id.tx,
                        },
                        amount: rune.amount as u64,
                    })
                    .collect(),
            });
        }

        Ok(rune_utxos)
    }
}
