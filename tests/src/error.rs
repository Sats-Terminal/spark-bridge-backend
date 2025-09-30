use thiserror::Error;
use crate::bitcoin_client::BitcoinClientError;

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
    // #[error("Failed to transfer spark")]
    // SparkAddressError(#[from] SparkAddressError),
    // #[error("Spark client error: {0}")]
    // SparkClientError(#[from] SparkClientError),
    #[error("Token identifier mismatch")]
    TokenIdentifierMismatch,
    #[error("Invalid data: {0}")]
    InvalidData(String),
    #[error("Failed to hash transaction: {0}")]
    HashError(String),
    #[error("Mint rune error: {0}")]
    MintRuneError(String),
    #[error("Get rune balance error: {0}")]
    GetRuneBalanceError(String),
}

