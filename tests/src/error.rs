use thiserror::Error;
use crate::bitcoin_client::BitcoinClientError;

#[derive(Error, Debug)]
pub enum TestError {
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
    #[error("Failed to mint rune")]
    MintRuneError(String),
    #[error("Failed to get balance")]
    GetRuneBalanceError(String)
}