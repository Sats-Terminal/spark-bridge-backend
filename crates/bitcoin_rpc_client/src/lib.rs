use std::{collections::HashMap, fmt, str::FromStr};

use bitcoin::{BlockHash, Txid, consensus};
use bitcoincore_rpc::{Auth, Client as CoreClient, RpcApi};
use bitcoincore_rpc_json::GetBlockchainInfoResult;
use futures::executor::block_on;
use hex::FromHex;
use reqwest::{
    Client,
    header::{HeaderMap, HeaderValue},
};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use thiserror::Error;
use url::Url;

pub type Result<T> = std::result::Result<T, BitcoinRpcClientError>;

pub trait BitcoinRpcClient: Send + Sync {
    fn get_transaction(&self, txid: &Txid) -> Result<bitcoin::Transaction>;
    fn get_blockchain_info(&self) -> Result<GetBlockchainInfoResult>;
    fn send_raw_transaction(&self, tx_hex: &str) -> Result<Txid>;
}

#[derive(Debug, Error)]
pub enum BitcoinRpcClientError {
    #[error("bitcoincore rpc error: {0}")]
    CoreClient(#[from] bitcoincore_rpc::Error),
    #[error("reqwest error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("serde json error: {0}")]
    SerdeJson(#[from] serde_json::Error),
    #[error("url parse error: {0}")]
    UrlParse(#[from] url::ParseError),
    #[error("decode error: {0}")]
    Decode(String),
    #[error("maestro returned error: {status} {message}")]
    MaestroError {
        status: reqwest::StatusCode,
        message: String,
    },
    #[error("missing field in response: {0}")]
    MissingField(&'static str),
}

pub struct CoreBitcoinRpcClient {
    inner: CoreClient,
}

impl CoreBitcoinRpcClient {
    pub fn new(url: &str, auth: Auth) -> Result<Self> {
        Ok(Self {
            inner: CoreClient::new(url, auth)?,
        })
    }

    pub fn inner(&self) -> &CoreClient {
        &self.inner
    }
}

impl BitcoinRpcClient for CoreBitcoinRpcClient {
    fn get_transaction(&self, txid: &Txid) -> Result<bitcoin::Transaction> {
        Ok(self.inner.get_raw_transaction(txid, None)?)
    }

    fn get_blockchain_info(&self) -> Result<GetBlockchainInfoResult> {
        Ok(self.inner.get_blockchain_info()?)
    }

    fn send_raw_transaction(&self, tx_hex: &str) -> Result<Txid> {
        let bytes = Vec::<u8>::from_hex(tx_hex).map_err(|err| BitcoinRpcClientError::Decode(err.to_string()))?;
        Ok(self.inner.send_raw_transaction(&bytes)?)
    }
}

pub struct MaestroBitcoinRpcClient {
    http: Client,
    base_url: Url,
}

impl MaestroBitcoinRpcClient {
    const API_PREFIX: &'static str = "v0/rpc/";

    pub fn new(mut base_url: Url, api_key: String) -> Result<Self> {
        if !base_url.path().ends_with(Self::API_PREFIX) {
            base_url = base_url
                .join(Self::API_PREFIX)
                .map_err(BitcoinRpcClientError::UrlParse)?;
        }

        let mut default_headers = HeaderMap::new();
        let api_key_value =
            HeaderValue::from_str(api_key.as_str()).map_err(|err| BitcoinRpcClientError::Decode(err.to_string()))?;
        default_headers.insert("api-key", api_key_value);

        let http = Client::builder()
            .default_headers(default_headers)
            .build()
            .map_err(BitcoinRpcClientError::Http)?;

        Ok(Self { http, base_url })
    }

    fn endpoint(&self, path: &str) -> Result<Url> {
        self.base_url
            .join(path.trim_start_matches('/'))
            .map_err(BitcoinRpcClientError::UrlParse)
    }

    async fn get<T>(&self, path: &str) -> Result<T>
    where
        T: DeserializeOwned,
    {
        let url = self.endpoint(path)?;
        let response = self.http.get(url).send().await?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(BitcoinRpcClientError::MaestroError { status, message: body });
        }
        let envelope = response.json::<ApiResponse<T>>().await?;
        Ok(envelope.data)
    }

    async fn post<T, U>(&self, path: &str, body: &T) -> Result<U>
    where
        T: serde::Serialize,
        U: DeserializeOwned,
    {
        let url = self.endpoint(path)?;
        let response = self.http.post(url).json(body).send().await?;
        let status = response.status();
        if !status.is_success() {
            let message = response.text().await.unwrap_or_default();
            return Err(BitcoinRpcClientError::MaestroError { status, message });
        }
        let envelope = response.json::<ApiResponse<U>>().await?;
        Ok(envelope.data)
    }
}

impl BitcoinRpcClient for MaestroBitcoinRpcClient {
    fn get_transaction(&self, txid: &Txid) -> Result<bitcoin::Transaction> {
        let path = format!("transaction/{txid}?verbose=true");
        let data = block_on(self.get::<MaestroTransactionResponse>(&path))?;
        let hex = data.hex.ok_or(BitcoinRpcClientError::MissingField("hex"))?;
        let bytes = Vec::<u8>::from_hex(&hex).map_err(|err| BitcoinRpcClientError::Decode(err.to_string()))?;
        Ok(consensus::encode::deserialize(&bytes).map_err(|err| BitcoinRpcClientError::Decode(err.to_string()))?)
    }

    fn get_blockchain_info(&self) -> Result<GetBlockchainInfoResult> {
        let info = block_on(self.get::<MaestroBlockchainInfoResponse>("general/info"))?;
        info.try_into()
    }

    fn send_raw_transaction(&self, tx_hex: &str) -> Result<Txid> {
        #[derive(serde::Serialize)]
        struct SubmitRequest<'a> {
            tx_hex: &'a str,
        }

        #[derive(Deserialize)]
        struct SubmitResponse {
            txid: String,
        }

        let response: SubmitResponse = block_on(self.post("transaction/submit", &SubmitRequest { tx_hex }))?;
        Txid::from_str(&response.txid).map_err(|err| BitcoinRpcClientError::Decode(err.to_string()))
    }
}

#[derive(Deserialize)]
struct ApiResponse<T> {
    data: T,
}

#[derive(Deserialize)]
struct MaestroTransactionResponse {
    hex: Option<String>,
}

#[derive(Deserialize)]
struct MaestroBlockchainInfoResponse {
    chain: String,
    blocks: u64,
    headers: u64,
    #[serde(rename = "bestblockhash")]
    best_block_hash: String,
    difficulty: f64,
    #[serde(rename = "mediantime")]
    median_time: u64,
    #[serde(rename = "verificationprogress")]
    verification_progress: f64,
    #[serde(rename = "initialblockdownload")]
    initial_block_download: bool,
    #[serde(rename = "chainwork")]
    chain_work: String,
    #[serde(rename = "size_on_disk")]
    size_on_disk: u64,
    pruned: bool,
    #[serde(rename = "pruneheight")]
    prune_height: Option<u64>,
    #[serde(rename = "automatic_pruning")]
    automatic_pruning: Option<bool>,
    #[serde(rename = "prune_target_size")]
    prune_target_size: Option<u64>,
    #[serde(default)]
    softforks: HashMap<String, MaestroSoftfork>,
    warnings: Option<Warnings>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Warnings {
    String(String),
    Array(Vec<String>),
}

#[derive(Deserialize)]
struct MaestroSoftfork {
    #[serde(rename = "type")]
    type_: Option<String>,
    active: bool,
    height: Option<u32>,
    bip9: Option<MaestroBip9SoftforkInfo>,
}

#[derive(Deserialize)]
struct MaestroBip9SoftforkInfo {
    status: Option<String>,
    bit: Option<u8>,
    start_time: Option<i64>,
    timeout: Option<u64>,
    since: Option<u32>,
    statistics: Option<MaestroBip9SoftforkStatistics>,
}

#[derive(Deserialize)]
struct MaestroBip9SoftforkStatistics {
    period: Option<u32>,
    threshold: Option<u32>,
    elapsed: Option<u32>,
    count: Option<u32>,
    possible: Option<bool>,
}

impl TryFrom<MaestroBlockchainInfoResponse> for GetBlockchainInfoResult {
    type Error = BitcoinRpcClientError;

    fn try_from(value: MaestroBlockchainInfoResponse) -> Result<Self> {
        let chain = match value.chain.as_str() {
            "main" | "mainnet" => bitcoin::Network::Bitcoin,
            "test" | "testnet" => bitcoin::Network::Testnet,
            "regtest" => bitcoin::Network::Regtest,
            "signet" => bitcoin::Network::Signet,
            other => bitcoin::Network::from_str(other).map_err(|err| BitcoinRpcClientError::Decode(err.to_string()))?,
        };

        let chain_work = hex::decode(value.chain_work.trim_start_matches("0x"))
            .map_err(|err| BitcoinRpcClientError::Decode(err.to_string()))?;

        let best_block_hash = BlockHash::from_str(&value.best_block_hash)
            .map_err(|err| BitcoinRpcClientError::Decode(err.to_string()))?;

        let mut softforks = HashMap::new();
        for (name, fork) in value.softforks {
            if let Some(sf) = convert_softfork(fork)? {
                softforks.insert(name, sf);
            }
        }

        let warnings = match value.warnings {
            Some(Warnings::String(s)) => bitcoincore_rpc_json::StringOrStringArray::String(s),
            Some(Warnings::Array(arr)) => bitcoincore_rpc_json::StringOrStringArray::StringArray(arr),
            None => bitcoincore_rpc_json::StringOrStringArray::String(String::new()),
        };

        Ok(GetBlockchainInfoResult {
            chain,
            blocks: value.blocks,
            headers: value.headers,
            best_block_hash,
            difficulty: value.difficulty,
            median_time: value.median_time,
            verification_progress: value.verification_progress,
            initial_block_download: value.initial_block_download,
            chain_work,
            size_on_disk: value.size_on_disk,
            pruned: value.pruned,
            prune_height: value.prune_height,
            automatic_pruning: value.automatic_pruning,
            prune_target_size: value.prune_target_size,
            softforks,
            warnings,
        })
    }
}

fn convert_softfork(value: MaestroSoftfork) -> Result<Option<bitcoincore_rpc_json::Softfork>> {
    let type_ = match value.type_.as_deref() {
        Some("buried") => Some(bitcoincore_rpc_json::SoftforkType::Buried),
        Some("bip9") => Some(bitcoincore_rpc_json::SoftforkType::Bip9),
        Some(_) => Some(bitcoincore_rpc_json::SoftforkType::Other),
        None => None,
    };

    let bip9 = value.bip9.map(|info| bitcoincore_rpc_json::Bip9SoftforkInfo {
        status: info
            .status
            .as_deref()
            .map(bip9_status_from_str)
            .unwrap_or(bitcoincore_rpc_json::Bip9SoftforkStatus::Defined),
        bit: info.bit,
        start_time: info.start_time.unwrap_or_default(),
        timeout: info.timeout.unwrap_or_default(),
        since: info.since.unwrap_or_default(),
        statistics: info
            .statistics
            .map(|stats| bitcoincore_rpc_json::Bip9SoftforkStatistics {
                period: stats.period.unwrap_or_default(),
                threshold: stats.threshold,
                elapsed: stats.elapsed.unwrap_or_default(),
                count: stats.count.unwrap_or_default(),
                possible: stats.possible,
            }),
    });

    Ok(type_.map(|type_| bitcoincore_rpc_json::Softfork {
        type_,
        bip9,
        height: value.height,
        active: value.active,
    }))
}

fn bip9_status_from_str(value: &str) -> bitcoincore_rpc_json::Bip9SoftforkStatus {
    use bitcoincore_rpc_json::Bip9SoftforkStatus::*;
    match value.to_lowercase().as_str() {
        "defined" => Defined,
        "started" => Started,
        "locked_in" => LockedIn,
        "active" => Active,
        "failed" => Failed,
        _ => Defined,
    }
}

impl fmt::Debug for dyn BitcoinRpcClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BitcoinRpcClient")
    }
}
