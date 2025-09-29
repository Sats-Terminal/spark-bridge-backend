use thiserror::Error;

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
}

#[derive(Error, Debug)]
pub enum BitcoinClientError {
    #[error("Failed to make bitcoin client call: {0}")]
    BitcoinRpcError(#[from] bitcoincore_rpc::Error),
    #[error("Decode error: {0}")]
    DecodeError(String),
    #[error("Failed to make titan client call: {0}")]
    TitanRpcError(#[from] titan_client::Error),
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
    TonicRequestError(#[from] tonic::Status),
    #[error("Decode error: {0}")]
    DecodeError(String),
}
