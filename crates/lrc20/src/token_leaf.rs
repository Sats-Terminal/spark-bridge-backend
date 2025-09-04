use bitcoin::{
    BlockHash, Txid, absolute::LockTime as AbsoluteLocktime, hashes::sha256::Hash, secp256k1,
};

use crate::token_identifier::TokenIdentifier;

/// Represents a token leaf to spend in a token transaction.
///
///
/// This struct contains the parent leaf hash and index of the leaf to spend.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TokenLeafToSpend {
    /// The hash of the parent leaf.
    pub parent_leaf_hash: Hash,

    /// The index of the parent leaf.
    pub parent_leaf_index: u32,
}

/// Represents the data structure for a Spark LRC-20 token leaf node.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TokenLeafOutput {
    /// The ID of the leaf.
    pub id: Option<String>,

    /// The owner's public key.
    pub owner_public_key: secp256k1::PublicKey,

    /// The revocation public key.
    pub revocation_public_key: secp256k1::PublicKey,

    /// The token public key.
    pub token_identifier: TokenIdentifier,

    /// The token amount.
    pub token_amount: u128,

    /// The withdrawal bond in sats.
    pub withdrawal_bond_sats: Option<u64>,

    /// The withdrawal locktime.
    pub withdrawal_locktime: Option<AbsoluteLocktime>,

    /// Whether the leaf is frozen.
    pub is_frozen: Option<bool>,

    /// The withdrawal transaction ID.
    pub withdraw_txid: Option<Txid>,

    /// The withdrawal transaction output index.
    pub withdraw_tx_vout: Option<u32>,

    /// The withdrawal block height.
    pub withdraw_height: Option<u32>,

    /// The withdrawal block hash.
    pub withdraw_block_hash: Option<BlockHash>,
}
