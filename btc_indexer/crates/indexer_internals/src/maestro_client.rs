use std::{collections::HashMap, str::FromStr};

use async_trait::async_trait;
use bitcoin::CompactTarget;
use bitcoin::hash_types::TxMerkleNode;
use bitcoin::network::Network;
use bitcoin::{
    BlockHash, Txid,
    block::{Header, Version},
    consensus,
};
use config_parser::config::MaestroConfig;
use reqwest::{
    Client, StatusCode, Url,
    header::{HeaderMap, HeaderValue},
};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use titan_client::{Error, TitanApi};
use titan_types::{
    AddressData, AddressTxOut, Block, BlockTip, RuneAmount, RuneId, RuneResponse, SerializedTxid, SpacedRune,
    SpentStatus, Status, Transaction, TransactionStatus, TxIn, TxOut,
};

use crate::error::BtcIndexerError;

#[derive(Clone)]
pub struct MaestroClient {
    http: Client,
    indexer_base: Url,
    rpc_base: Url,
    network: Network,
}

impl MaestroClient {
    pub fn new(config: &MaestroConfig, network: Network) -> Result<Self, BtcIndexerError> {
        let mut headers = HeaderMap::new();
        headers.insert(
            "api-key",
            HeaderValue::from_str(config.api_key.as_str())
                .map_err(|err| BtcIndexerError::MaestroClientError(format!("invalid Maestro API key: {err}")))?,
        );
        let http = Client::builder()
            .default_headers(headers)
            .build()
            .map_err(|err| BtcIndexerError::MaestroClientError(err.to_string()))?;

        let indexer_base = config
            .base_url
            .join("v0/")
            .map_err(|err| BtcIndexerError::MaestroClientError(err.to_string()))?;
        let rpc_base = indexer_base
            .join("rpc/")
            .map_err(|err| BtcIndexerError::MaestroClientError(err.to_string()))?;

        Ok(Self {
            http,
            indexer_base,
            rpc_base,
            network,
        })
    }

    async fn get_indexer<T>(&self, path: &str, query: Option<&[(&str, &str)]>) -> Result<T, Error>
    where
        T: DeserializeOwned,
    {
        self.get_json(&self.indexer_base, path, query).await
    }

    async fn get_rpc<T>(&self, path: &str, query: Option<&[(&str, &str)]>) -> Result<T, Error>
    where
        T: DeserializeOwned,
    {
        self.get_json(&self.rpc_base, path, query).await
    }

    async fn get_json<T>(&self, base: &Url, path: &str, query: Option<&[(&str, &str)]>) -> Result<T, Error>
    where
        T: DeserializeOwned,
    {
        let url = base
            .join(path.trim_start_matches('/'))
            .map_err(|err| Error::TitanError(StatusCode::BAD_REQUEST, err.to_string()))?;
        let mut request = self.http.get(url);
        if let Some(params) = query {
            request = request.query(params);
        }
        let response = request.send().await.map_err(Error::from)?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(Error::TitanError(status, body));
        }
        let envelope = response.json::<ApiResponse<T>>().await.map_err(Error::from)?;
        Ok(envelope.data)
    }

    fn parse_u64(value: &str) -> Result<u64, Error> {
        value
            .replace('_', "")
            .parse::<u64>()
            .map_err(|err| Error::TitanError(StatusCode::UNPROCESSABLE_ENTITY, err.to_string()))
    }

    fn parse_decimal_amount(value: &str) -> Result<u128, Error> {
        let mut parts = value.splitn(2, '.');
        let int_part = parts.next().unwrap_or_default();
        let frac_part = parts.next().unwrap_or("");
        if frac_part.trim_end_matches('0').is_empty() {
            let combined = format!("{}{}", int_part, frac_part).replace('_', "");
            let digits = if combined.is_empty() { "0" } else { combined.as_str() };
            digits
                .parse::<u128>()
                .map_err(|err| Error::TitanError(StatusCode::UNPROCESSABLE_ENTITY, err.to_string()))
        } else {
            Err(Error::TitanError(
                StatusCode::UNPROCESSABLE_ENTITY,
                format!("fractional rune amounts are unsupported: {value}"),
            ))
        }
    }

    fn parse_rune_id(value: &str) -> Result<RuneId, Error> {
        RuneId::from_str(value)
            .map_err(|err| Error::TitanError(StatusCode::UNPROCESSABLE_ENTITY, format!("invalid rune id: {err}")))
    }
}

#[async_trait]
impl TitanApi for MaestroClient {
    async fn get_status(&self) -> Result<Status, Error> {
        let latest = self
            .get_rpc::<BlockHeaderResponse>("block/latest", Some(&[("count", "1"), ("page", "1")]))
            .await?;
        Ok(Status {
            block_tip: BlockTip {
                height: latest.height,
                hash: latest.hash,
                is_at_tip: true,
            },
            runes_count: 0,
            mempool_tx_count: 0,
        })
    }

    async fn get_tip(&self) -> Result<BlockTip, Error> {
        let latest = self
            .get_rpc::<BlockHeaderResponse>("block/latest", Some(&[("count", "1"), ("page", "1")]))
            .await?;
        Ok(BlockTip {
            height: latest.height,
            hash: latest.hash,
            is_at_tip: true,
        })
    }

    async fn get_block(&self, block: &titan_types::query::Block) -> Result<Block, Error> {
        let identifier = match block {
            titan_types::query::Block::Height(height) => height.to_string(),
            titan_types::query::Block::Hash(hash) => hash.to_string(),
        };
        let data = self
            .get_rpc::<RpcBlockResponse>(&format!("block/{identifier}"), Some(&[("verbose", "false")]))
            .await?;

        let version = Version::from_consensus(data.version);
        let prev_blockhash = BlockHash::from_str(&data.previousblockhash)
            .map_err(|err| Error::TitanError(StatusCode::UNPROCESSABLE_ENTITY, format!("invalid block hash: {err}")))?;
        let merkle_root = TxMerkleNode::from_str(&data.merkleroot).map_err(|err| {
            Error::TitanError(StatusCode::UNPROCESSABLE_ENTITY, format!("invalid merkle root: {err}"))
        })?;
        let bits_value = u32::from_str_radix(data.bits.trim_start_matches("0x"), 16)
            .map_err(|err| Error::TitanError(StatusCode::UNPROCESSABLE_ENTITY, format!("invalid bits: {err}")))?;

        let header = Header {
            version,
            prev_blockhash,
            merkle_root,
            time: data.time as u32,
            bits: CompactTarget::from_consensus(bits_value),
            nonce: data.nonce,
        };

        let tx_ids = data
            .tx
            .into_iter()
            .map(|txid| SerializedTxid::from_str(&txid))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| Error::TitanError(StatusCode::UNPROCESSABLE_ENTITY, format!("invalid txid: {err}")))?;

        Ok(Block {
            height: data.height,
            header,
            tx_ids,
            etched_runes: Vec::new(),
        })
    }

    async fn get_block_hash_by_height(&self, height: u64) -> Result<String, Error> {
        let response = self
            .get_rpc::<BlockHeaderResponse>(&format!("block/{height}"), Some(&[("verbose", "false")]))
            .await?;
        Ok(response.hash)
    }

    async fn get_block_txids(&self, block: &titan_types::query::Block) -> Result<Vec<String>, Error> {
        let block = self.get_block(block).await?;
        Ok(block.tx_ids.into_iter().map(|txid| txid.to_string()).collect())
    }

    async fn get_address(&self, address: &str) -> Result<AddressData, Error> {
        let mut params = vec![("count", "100")];
        if matches!(self.network, Network::Regtest) {
            params.push(("order", "asc"));
        }
        let utxos: Vec<AddressUtxo> = self
            .get_indexer(&format!("addresses/{address}/utxos"), Some(&params))
            .await?;

        let mut value_sum = 0u64;
        let mut outputs = Vec::with_capacity(utxos.len());
        let mut rune_totals: HashMap<RuneId, u128> = HashMap::new();

        for utxo in utxos {
            let value = Self::parse_u64(&utxo.satoshis)?;
            value_sum = value_sum.saturating_add(value);

            let runes = utxo
                .runes
                .iter()
                .map(|r| {
                    let id = Self::parse_rune_id(&r.rune_id)?;
                    let amount = Self::parse_decimal_amount(&r.amount)?;
                    rune_totals
                        .entry(id)
                        .and_modify(|acc| *acc = acc.saturating_add(amount))
                        .or_insert(amount);
                    Ok(RuneAmount { rune_id: id, amount })
                })
                .collect::<Result<Vec<_>, Error>>()?;

            let txid = Txid::from_str(&utxo.txid)
                .map_err(|err| Error::TitanError(StatusCode::UNPROCESSABLE_ENTITY, format!("invalid txid: {err}")))?;

            let status = if utxo.confirmations.unwrap_or(0) > 0 {
                TransactionStatus {
                    confirmed: true,
                    block_height: utxo.height,
                    block_hash: None,
                }
            } else {
                TransactionStatus::unconfirmed()
            };

            outputs.push(AddressTxOut {
                txid,
                vout: utxo.vout,
                value,
                runes,
                risky_runes: Vec::new(),
                spent: SpentStatus::Unspent,
                status,
                size: 0,
                weight: 0,
            });
        }

        let aggregated_runes = rune_totals
            .into_iter()
            .map(|(id, amount)| RuneAmount { rune_id: id, amount })
            .collect();

        Ok(AddressData {
            value: value_sum,
            runes: aggregated_runes,
            outputs,
        })
    }

    async fn get_transaction(&self, txid: &Txid) -> Result<Transaction, Error> {
        let txid_str = txid.to_string();
        let meta: IndexerTransaction = self
            .get_indexer(&format!("transactions/{txid_str}/metaprotocols"), None)
            .await?;
        let rpc: RpcTransaction = self
            .get_rpc(&format!("transaction/{txid_str}"), Some(&[("verbose", "true")]))
            .await?;

        let bytes = hex::decode(&rpc.hex)
            .map_err(|err| Error::TitanError(StatusCode::UNPROCESSABLE_ENTITY, format!("invalid tx hex: {err}")))?;
        let btc_tx: bitcoin::Transaction = consensus::deserialize(&bytes).map_err(Error::from)?;

        let mut inputs = Vec::with_capacity(btc_tx.input.len());
        for (idx, input) in btc_tx.input.iter().enumerate() {
            let prev_output = meta.inputs.get(idx).and_then(|m| {
                let satoshis = m.satoshis.as_ref().and_then(|value| Self::parse_u64(value).ok());
                satoshis.map(|value| TxOut {
                    value,
                    script_pubkey: bitcoin::ScriptBuf::new(),
                    runes: Vec::new(),
                    risky_runes: Vec::new(),
                    spent: SpentStatus::SpentUnknown,
                })
            });
            inputs.push(TxIn::from((input.clone(), prev_output)));
        }

        let mut outputs = Vec::with_capacity(btc_tx.output.len());
        for (idx, output) in btc_tx.output.iter().enumerate() {
            let runes = meta
                .outputs
                .get(idx)
                .map(|out| {
                    out.runes
                        .iter()
                        .map(|r| {
                            Ok(RuneAmount {
                                rune_id: Self::parse_rune_id(&r.rune_id)?,
                                amount: Self::parse_decimal_amount(&r.amount)?,
                            })
                        })
                        .collect::<Result<Vec<_>, Error>>()
                })
                .transpose()?
                .unwrap_or_default();

            let spent = if meta.outputs.get(idx).and_then(|out| out.spending_tx.as_ref()).is_some() {
                SpentStatus::SpentUnknown
            } else {
                SpentStatus::Unspent
            };

            outputs.push(TxOut {
                value: output.value.to_sat(),
                script_pubkey: output.script_pubkey.clone(),
                runes,
                risky_runes: Vec::new(),
                spent,
            });
        }

        let confirmed = meta.confirmations.unwrap_or(0) > 0;
        let block_hash = meta
            .block_hash
            .or(rpc.blockhash.clone())
            .map(|hash| BlockHash::from_str(&hash))
            .transpose()
            .map_err(|err| Error::TitanError(StatusCode::UNPROCESSABLE_ENTITY, format!("invalid block hash: {err}")))?;

        let status = if confirmed {
            TransactionStatus {
                confirmed: true,
                block_height: meta.height.or(rpc.blockheight),
                block_hash,
            }
        } else {
            TransactionStatus::unconfirmed()
        };

        Ok(Transaction {
            txid: txid.clone(),
            version: rpc.version,
            lock_time: rpc.locktime,
            input: inputs,
            output: outputs,
            status,
            size: rpc.size as u64,
            weight: rpc.weight as u64,
        })
    }

    async fn get_transaction_raw(&self, _txid: &Txid) -> Result<Vec<u8>, Error> {
        Err(Error::TitanError(
            StatusCode::NOT_IMPLEMENTED,
            "get_transaction_raw is not implemented for Maestro client".to_string(),
        ))
    }

    async fn get_transaction_hex(&self, _txid: &Txid) -> Result<String, Error> {
        Err(Error::TitanError(
            StatusCode::NOT_IMPLEMENTED,
            "get_transaction_hex is not implemented for Maestro client".to_string(),
        ))
    }

    async fn get_transaction_status(&self, txid: &Txid) -> Result<TransactionStatus, Error> {
        Ok(self.get_transaction(txid).await?.status)
    }

    async fn send_transaction(&self, _tx_hex: String) -> Result<Txid, Error> {
        Err(Error::TitanError(
            StatusCode::NOT_IMPLEMENTED,
            "send_transaction is not implemented for Maestro client".to_string(),
        ))
    }

    async fn get_output(&self, _outpoint: &bitcoin::OutPoint) -> Result<TxOut, Error> {
        Err(Error::TitanError(
            StatusCode::NOT_IMPLEMENTED,
            "get_output is not implemented for Maestro client".to_string(),
        ))
    }

    async fn get_inscription(
        &self,
        _inscription_id: &titan_types::InscriptionId,
    ) -> Result<(reqwest::header::HeaderMap, Vec<u8>), Error> {
        Err(Error::TitanError(
            StatusCode::NOT_IMPLEMENTED,
            "get_inscription is not implemented for Maestro client".to_string(),
        ))
    }

    async fn get_runes(
        &self,
        _pagination: Option<titan_types::Pagination>,
    ) -> Result<titan_types::PaginationResponse<RuneResponse>, Error> {
        Err(Error::TitanError(
            StatusCode::NOT_IMPLEMENTED,
            "get_runes is not implemented for Maestro client".to_string(),
        ))
    }

    async fn get_rune(&self, rune: &titan_types::query::Rune) -> Result<RuneResponse, Error> {
        let identifier = rune.to_string();
        let data: RuneData = self.get_indexer(&format!("assets/runes/{identifier}"), None).await?;

        let rune_id = Self::parse_rune_id(&data.id)?;
        let etching = SerializedTxid::from_str(&data.etching_tx)
            .map_err(|err| Error::TitanError(StatusCode::UNPROCESSABLE_ENTITY, format!("invalid etching tx: {err}")))?;
        let spaced_rune = SpacedRune::from_str(&data.spaced_name).map_err(|err| {
            Error::TitanError(StatusCode::UNPROCESSABLE_ENTITY, format!("invalid spaced rune: {err}"))
        })?;

        Ok(RuneResponse {
            id: rune_id,
            block: data.etching_height,
            burned: 0,
            divisibility: data.divisibility,
            etching,
            number: 0,
            premine: Self::parse_decimal_amount(&data.premine)?,
            supply: Self::parse_decimal_amount(&data.circulating_supply)?,
            max_supply: Self::parse_decimal_amount(&data.max_supply)?,
            spaced_rune,
            symbol: data.symbol.and_then(|s| s.chars().next()),
            mint: None,
            burns: 0,
            pending_burns: 0,
            pending_mints: 0,
            inscription_id: None,
            timestamp: 0,
            turbo: false,
        })
    }

    async fn get_rune_transactions(
        &self,
        _rune: &titan_types::query::Rune,
        _pagination: Option<titan_types::Pagination>,
    ) -> Result<titan_types::PaginationResponse<Txid>, Error> {
        Err(Error::TitanError(
            StatusCode::NOT_IMPLEMENTED,
            "get_rune_transactions is not implemented for Maestro client".to_string(),
        ))
    }

    async fn get_mempool_txids(&self) -> Result<Vec<Txid>, Error> {
        Err(Error::TitanError(
            StatusCode::NOT_IMPLEMENTED,
            "get_mempool_txids is not implemented for Maestro client".to_string(),
        ))
    }

    async fn get_mempool_entry(&self, _txid: &Txid) -> Result<titan_types::MempoolEntry, Error> {
        Err(Error::TitanError(
            StatusCode::NOT_IMPLEMENTED,
            "get_mempool_entry is not implemented for Maestro client".to_string(),
        ))
    }

    async fn get_mempool_entries(
        &self,
        _txids: &[Txid],
    ) -> Result<HashMap<Txid, Option<titan_types::MempoolEntry>>, Error> {
        Err(Error::TitanError(
            StatusCode::NOT_IMPLEMENTED,
            "get_mempool_entries is not implemented for Maestro client".to_string(),
        ))
    }

    async fn get_all_mempool_entries(&self) -> Result<HashMap<Txid, titan_types::MempoolEntry>, Error> {
        Err(Error::TitanError(
            StatusCode::NOT_IMPLEMENTED,
            "get_all_mempool_entries is not implemented for Maestro client".to_string(),
        ))
    }

    async fn get_mempool_entries_with_ancestors(
        &self,
        _txids: &[Txid],
    ) -> Result<HashMap<Txid, titan_types::MempoolEntry>, Error> {
        Err(Error::TitanError(
            StatusCode::NOT_IMPLEMENTED,
            "get_mempool_entries_with_ancestors is not implemented for Maestro client".to_string(),
        ))
    }

    async fn get_subscription(&self, _id: &str) -> Result<titan_types::Subscription, Error> {
        Err(Error::TitanError(
            StatusCode::NOT_IMPLEMENTED,
            "Subscriptions are not implemented for Maestro client".to_string(),
        ))
    }

    async fn list_subscriptions(&self) -> Result<Vec<titan_types::Subscription>, Error> {
        Err(Error::TitanError(
            StatusCode::NOT_IMPLEMENTED,
            "Subscriptions are not implemented for Maestro client".to_string(),
        ))
    }

    async fn add_subscription(
        &self,
        _subscription: &titan_types::Subscription,
    ) -> Result<titan_types::Subscription, Error> {
        Err(Error::TitanError(
            StatusCode::NOT_IMPLEMENTED,
            "Subscriptions are not implemented for Maestro client".to_string(),
        ))
    }

    async fn delete_subscription(&self, _id: &str) -> Result<(), Error> {
        Err(Error::TitanError(
            StatusCode::NOT_IMPLEMENTED,
            "Subscriptions are not implemented for Maestro client".to_string(),
        ))
    }
}

#[derive(Deserialize)]
struct ApiResponse<T> {
    data: T,
}

#[derive(Deserialize)]
struct BlockHeaderResponse {
    hash: String,
    height: u64,
}

#[derive(Deserialize)]
struct RpcBlockResponse {
    hash: String,
    height: u64,
    version: i32,
    previousblockhash: String,
    merkleroot: String,
    time: u64,
    bits: String,
    nonce: u32,
    tx: Vec<String>,
}

#[derive(Deserialize)]
struct AddressUtxo {
    txid: String,
    vout: u32,
    satoshis: String,
    confirmations: Option<u64>,
    height: Option<u64>,
    runes: Vec<RuneJson>,
}

#[derive(Deserialize)]
struct RuneJson {
    rune_id: String,
    amount: String,
}

#[derive(Deserialize)]
struct IndexerTransaction {
    height: Option<u64>,
    block_hash: Option<String>,
    confirmations: Option<u64>,
    inputs: Vec<IndexerTxInput>,
    outputs: Vec<IndexerTxOutput>,
}

#[derive(Deserialize)]
struct IndexerTxInput {
    satoshis: Option<String>,
}

#[derive(Deserialize)]
struct IndexerTxOutput {
    runes: Vec<RuneJson>,
    spending_tx: Option<String>,
}

#[derive(Deserialize)]
struct RpcTransaction {
    version: i32,
    locktime: u32,
    size: u32,
    weight: u32,
    hex: String,
    blockhash: Option<String>,
    blockheight: Option<u64>,
}

#[derive(Deserialize)]
struct RuneData {
    id: String,
    etching_tx: String,
    etching_height: u64,
    spaced_name: String,
    symbol: Option<String>,
    divisibility: u8,
    premine: String,
    max_supply: String,
    circulating_supply: String,
}
