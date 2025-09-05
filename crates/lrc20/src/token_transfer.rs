use bitcoin::secp256k1::PublicKey;

use crate::{
    spark_hash::SparkHash,
    token_identifier::TokenIdentifier,
    token_leaf::{TokenLeafOutput, TokenLeafToSpend},
};
use serde::{Deserialize, Serialize};

/// The request for a token transfer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenTransfer {
    /// The pre-computed token transaction hash to use as transfer ID (if None, will be computed)
    pub transfer_hash: Option<SparkHash>,

    /// The leaves to spend.
    pub leaves_to_spend: Vec<(TokenLeafOutput, TokenLeafToSpend)>,

    /// The sender public key.
    pub sender_public_key: PublicKey,

    /// The receiver public key.
    pub receiver_public_key: PublicKey,

    /// The token identifier or token public key.
    pub token_identifier: TokenIdentifier,

    /// The amount.
    pub amount: u128,
}
