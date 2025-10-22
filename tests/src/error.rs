use spark_address::SparkAddressError;
use thiserror::Error;
use token_identifier::TokenIdentifierParseError;

#[derive(Error, Debug)]
pub enum RuneError {
    #[error("Failed to etch rune")]
    EtchRuneError(String),
    #[error("Bitcoin client error: {0}")]
    BitcoinClientError(#[from] BitcoinClientError),
    #[error("Failed to sign transaction")]
    SignTransactionError(String),
    #[error("Failed to unite unspent utxos")]
    UniteUnspentUtxosError(String),
    #[error("Failed to get funded outpoint")]
    GetFundedOutpointError(String),
    #[error("Insufficient balance")]
    InsufficientBalanceError(String),
    #[error("Failed to transfer runes")]
    TransferRunesError(String),
    #[error("Failed to transfer spark")]
    SparkAddressError(#[from] SparkAddressError),
    #[error("Spark client error: {0}")]
    SparkClientError(#[from] SparkClientError),
    #[error("Token identifier mismatch")]
    TokenIdentifierMismatch,
    #[error("Invalid data: {0}")]
    InvalidData(String),
    #[error("Failed to hash transaction: {0}")]
    HashError(String),
}

#[derive(Error, Debug)]
pub enum BitcoinClientError {
    #[error("Failed to make bitcoin client call: {0}")]
    BitcoinRpcError(#[from] bitcoincore_rpc::Error),
    #[error("Decode error: {0}")]
    DecodeError(String),
    #[error("BTC indexer client error: {0}")]
    BTCIndexerError(#[from] btc_indexer_client::error::BtcIndexerClientError),
}

#[derive(Error, Debug)]
pub enum GatewayClientError {
    #[error("Failed to join URL: {0}")]
    UrlJoinError(#[from] url::ParseError),
    #[error("Failed to send request: {0}")]
    SendRequestError(#[from] reqwest::Error),
    #[error("Error response: {0}")]
    ErrorResponse(String),
}

#[derive(Error, Debug)]
pub enum SparkClientError {
    #[error("Failed to create TLS channel: {0}")]
    CreateTlsChannelError(String),
    #[error("Failed to connect to operator: {0}")]
    ConnectionError(String),
    #[error("Failed to decode spark address: {0}")]
    DecodeSparkAddressError(#[from] spark_address::SparkAddressError),
    #[error("Tonic request error: {0}")]
    TonicRequestError(#[from] Box<tonic::Status>),
    #[error("Decode error: {0}")]
    DecodeError(String),
    #[error("Token identifier error: {0}")]
    TokenIdentifierError(#[from] TokenIdentifierParseError),
    #[error("Session token not found")]
    SessionTokenNotFound(String),
}
