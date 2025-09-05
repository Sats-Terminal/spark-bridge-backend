use std::ops::Deref;
use serde::{Deserialize, Serialize};
use bitcoin::hashes::{Hash, HashEngine, sha256::Hash as Sha256Hash};
use thiserror::Error;

use crate::{
    token_leaf::{TokenLeafOutput, TokenLeafToSpend},
    token_transaction::{TokenTransaction, TokenTransactionInput, TokenTransactionVersion},
};

/// A hash of the LRC20 receipt data that uniquely identifies a receipt (coin).
///
/// Defined as: `PXH = hash(hash(Y) || UV)`, where `Y` - is token_amount (amount),
/// and `UV` - is token type (issuer public key).
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SparkHash(pub Sha256Hash);

impl Deref for SparkHash {
    type Target = Sha256Hash;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<Sha256Hash> for SparkHash {
    fn from(hash: Sha256Hash) -> Self {
        Self(hash)
    }
}

impl From<[u8; 32]> for SparkHash {
    fn from(value: [u8; 32]) -> Self {
        Self(Sha256Hash::from_byte_array(value))
    }
}

impl SparkHash {
    /// Creates a `SparkHash` from a byte array.
    ///
    /// # Arguments
    ///
    /// * `hash_bytes` - The byte array to create the `SparkHash` from.
    pub fn from_hash_bytes(hash_bytes: &[u8; 32]) -> Self {
        Self(Sha256Hash::from_byte_array(*hash_bytes))
    }

    /// Hashes a token transaction following the LRC20 specification.
    ///
    /// # Arguments
    ///
    /// * `token_tx` - The token transaction to hash.
    /// * `is_partial_hash` - Whether to hash the transaction partially.
    ///
    /// # Returns
    ///
    /// A `SparkHash` representing the hash of the token transaction.
    pub fn hash_token_transaction(
        token_tx: &TokenTransaction,
        is_partial_hash: bool,
    ) -> Result<Self, SparkHashError> {
        let mut hash_engine = Sha256Hash::engine();

        let is_v2 = token_tx.version == TokenTransactionVersion::V2;

        if is_v2 {
            hash_engine.input(Sha256Hash::hash(&token_tx.version.bytes()).as_byte_array());
        }

        let input_type = match token_tx.input {
            TokenTransactionInput::Create(..) => 1u8,
            TokenTransactionInput::Mint(..) => 2u8,
            TokenTransactionInput::Transfer(..) => 3u8,
        };
        hash_engine.input(Sha256Hash::hash(&[0, 0, 0, input_type]).as_byte_array());

        // Hash inputs
        match &token_tx.input {
            TokenTransactionInput::Transfer(transfer_input) => {
                let inputs_len = transfer_input.leaves_to_spend.len() as u32;
                hash_engine.input(Sha256Hash::hash(&inputs_len.to_be_bytes()).as_byte_array());

                for leaf in &transfer_input.leaves_to_spend {
                    hash_engine.input(SparkHash::hash_token_leaf_to_spend(&leaf).0.as_byte_array());
                }
            },
            TokenTransactionInput::Mint(mint_input) => {
                hash_engine.input(
                    Sha256Hash::hash(&mint_input.issuer_public_key.serialize()).as_byte_array(),
                );

                if let Some(identifier) = mint_input.token_identifier {
                    hash_engine.input(Sha256Hash::hash(&identifier.to_bytes()).as_byte_array());
                }
            },
            TokenTransactionInput::Create(_) => {
                return Err(SparkHashError::InvalidTokenTransactionInput);
            },
        }

        let outputs_len = token_tx.leaves_to_create.len() as u32;
        hash_engine.input(Sha256Hash::hash(&outputs_len.to_be_bytes()).as_byte_array());

        // Hash output leaves
        for leaf in &token_tx.leaves_to_create {
            hash_engine.input(
                SparkHash::hash_token_leaf_output(leaf, is_partial_hash)?
                    .0
                    .as_byte_array(),
            );
        }

        let mut so_pubkeys = token_tx.spark_operator_identity_public_keys.clone();
        so_pubkeys.sort();

        // Hash spark operator identity public keys
        let so_pubkeys_len = so_pubkeys.len() as u32;
        hash_engine.input(Sha256Hash::hash(&so_pubkeys_len.to_be_bytes()).as_byte_array());

        for key in &so_pubkeys {
            hash_engine.input(Sha256Hash::hash(&key.serialize()).as_byte_array());
        }

        // Hash the network
        if let Some(network) = token_tx.network {
            hash_engine.input(Sha256Hash::hash(&network.to_be_bytes()).as_byte_array());
        }

        if is_v2 {
            hash_engine.input(
                Sha256Hash::hash(&token_tx.client_created_timestamp.to_be_bytes()).as_byte_array(),
            );

            if !is_partial_hash {
                hash_engine
                    .input(Sha256Hash::hash(&token_tx.expiry_time.to_be_bytes()).as_byte_array());
            }
        }

        Ok(Self(Sha256Hash::from_engine(hash_engine)))
    }

    /// Hashes a token leaf to spend following the LRC20 specification.
    ///
    /// # Arguments
    ///
    /// * `leaf` - The token leaf to spend.
    pub fn hash_token_leaf_to_spend(leaf: &TokenLeafToSpend) -> Self {
        let mut hash_engine = Sha256Hash::engine();

        hash_engine.input(leaf.parent_leaf_hash.as_byte_array());
        hash_engine.input(&leaf.parent_leaf_index.to_be_bytes());

        Self(Sha256Hash::from_engine(hash_engine))
    }

    /// Hashes a token leaf output following the LRC20 specification.
    ///
    /// # Arguments
    ///
    /// * `leaf` - The token leaf output to hash.
    /// * `is_partial_hash` - Whether to hash the leaf partially.
    pub fn hash_token_leaf_output(
        leaf: &TokenLeafOutput,
        is_partial_hash: bool,
    ) -> Result<Self, SparkHashError> {
        let mut hash_engine = Sha256Hash::engine();

        if !is_partial_hash && leaf.id.is_some() {
            hash_engine.input(leaf.id.as_ref().unwrap().as_bytes());
        }
        hash_engine.input(&leaf.owner_public_key.serialize());
        if !is_partial_hash {
            hash_engine.input(&leaf.revocation_public_key.serialize());
            hash_engine.input(
                &leaf
                    .withdrawal_bond_sats
                    .ok_or(SparkHashError::WithdrawalBondSatsMissing)?
                    .to_be_bytes(),
            );
            hash_engine.input(
                &(leaf
                    .withdrawal_locktime
                    .ok_or(SparkHashError::WithdrawalLocktimeMissing)?
                    .to_consensus_u32() as u64)
                    .to_be_bytes(),
            );
        }
        hash_engine.input(&[0u8; 33]);
        hash_engine.input(&leaf.token_identifier.to_bytes());
        hash_engine.input(&leaf.token_amount.to_be_bytes());

        Ok(Self(Sha256Hash::from_engine(hash_engine)))
    }
}

impl TryFrom<&TokenTransaction> for SparkHash {
    type Error = SparkHashError;

    fn try_from(token_tx: &TokenTransaction) -> Result<Self, Self::Error> {
        Self::hash_token_transaction(token_tx, false)
    }
}

/// Errors that can occur when working with `SparkHash` operations.
#[derive(Error, Debug)]
pub enum SparkHashError {
    /// The token transaction input is invalid.
    #[error("Invalid token transaction input: Token transaction V0 can't have")]
    InvalidTokenTransactionInput,

    /// The withdrawal bond sats is missing.
    #[error("Withdrawal bond sats is missing")]
    WithdrawalBondSatsMissing,

    /// The withdrawal locktime is missing.
    #[error("Withdrawal locktime is missing")]
    WithdrawalLocktimeMissing,

    /// Crate input parsing not yet implemented.
    #[error("Crate input parsing not yet implemented")]
    CreateInputNotImplemented,
}

#[cfg(test)]
mod test {
    use core::str::FromStr;

    use bitcoin::{Network, hashes::sha256::Hash, secp256k1};
    use once_cell::sync::Lazy;

    use super::SparkHash;
    use crate::{
        token_identifier::TokenIdentifier,
        token_leaf::{TokenLeafOutput, TokenLeafToSpend},
        token_transaction::{
            TokenTransaction, TokenTransactionInput, TokenTransactionMintInput,
            TokenTransactionTransferInput, TokenTransactionVersion,
        },
    };

    static TOKEN_IDENTIFIER: Lazy<TokenIdentifier> = Lazy::new(|| {
        TokenIdentifier::from_str(
            "0707070707070707070707070707070707070707070707070707070707070707",
            Network::Regtest,
        )
        .unwrap()
    });

    static ISSUER_PUBKEY: Lazy<secp256k1::PublicKey> = Lazy::new(|| {
        secp256k1::PublicKey::from_str(
            "02f29bd05a48d378f445631c6595de7b32fc3f633689e207e0a37a5df82a9fad2d",
        )
        .unwrap()
    });

    static IDENTITY_PUBKEY: Lazy<secp256k1::PublicKey> = Lazy::new(|| {
        secp256k1::PublicKey::from_str(
            "03199bd05a48d378f445631c6595de7b32fc3f633689e207e0a37a5df82a9fad2e",
        )
        .unwrap()
    });

    static REVOCATION_COMMITMENT: Lazy<secp256k1::PublicKey> = Lazy::new(|| {
        secp256k1::PublicKey::from_str(
            "02649bd05a48d378f445631c6595de7b32fc3f633689e207e0a37a5df82a9fad2e",
        )
        .unwrap()
    });

    static SO_PUBKEY: Lazy<secp256k1::PublicKey> = Lazy::new(|| {
        secp256k1::PublicKey::from_str(
            "03c89bd05a48d378f445631c6595de7b32fc3f633689e207e0a37a5df82a9fad2e",
        )
        .unwrap()
    });

    #[test]
    fn test_issue_token_tx_hash_v2() -> Result<(), Box<dyn std::error::Error>> {
        let token_amount = 1000;

        let token_tx = TokenTransaction {
            version: TokenTransactionVersion::V2,
            input: TokenTransactionInput::Mint(TokenTransactionMintInput {
                issuer_public_key: *ISSUER_PUBKEY,
                issuer_signature: None,
                issuer_provided_timestamp: None,
                token_identifier: Some(*TOKEN_IDENTIFIER),
            }),
            leaves_to_create: vec![TokenLeafOutput {
                id: Some("db1a4e48-0fc5-4f6c-8a80-d9d6c561a436".into()),
                owner_public_key: *IDENTITY_PUBKEY,
                revocation_public_key: *REVOCATION_COMMITMENT,
                withdrawal_bond_sats: Some(10000),
                withdrawal_locktime: Some(bitcoin::absolute::LockTime::from_height(100).unwrap()),
                token_identifier: *TOKEN_IDENTIFIER,
                token_amount,
                is_frozen: Some(false),
                withdraw_height: None,
                withdraw_txid: None,
                withdraw_tx_vout: None,
                withdraw_block_hash: None,
            }],
            spark_operator_identity_public_keys: vec![*SO_PUBKEY],
            expiry_time: 0,
            network: Some(2),
            client_created_timestamp: 1000,
        };

        let partial_spark_hash = SparkHash::hash_token_transaction(&token_tx, true)?;
        let final_spark_hash = SparkHash::hash_token_transaction(&token_tx, false)?;

        assert_eq!(
            partial_spark_hash.0,
            Hash::from_str("18c5c41161a9634c4c36ccad955ca5ce50dc49364413a4d4daff341da2070b1c")
                .unwrap()
        );
        assert_eq!(
            final_spark_hash.0,
            Hash::from_str("8cdda04a32fb32e2d70d1d3d8a63439a370ab9006b6d01c44e04732e5d84262f")
                .unwrap()
        );

        Ok(())
    }

    #[test]
    fn test_transfer_token_tx_hash_v2() -> Result<(), Box<dyn std::error::Error>> {
        let token_amount = 1000;

        let token_tx = TokenTransaction {
            version: TokenTransactionVersion::V2,
            input: TokenTransactionInput::Transfer(TokenTransactionTransferInput {
                leaves_to_spend: vec![TokenLeafToSpend {
                    parent_leaf_hash: Hash::from_str(
                        "456edc21a3c224dbbbf4ef97292d7acfd36b241c52aaf55c6f3b7ed59712df77",
                    )
                    .unwrap(),
                    parent_leaf_index: 0,
                }],
            }),
            leaves_to_create: vec![TokenLeafOutput {
                id: Some("db1a4e48-0fc5-4f6c-8a80-d9d6c561a436".into()),
                owner_public_key: *IDENTITY_PUBKEY,
                revocation_public_key: *REVOCATION_COMMITMENT,
                withdrawal_bond_sats: Some(10000),
                withdrawal_locktime: Some(bitcoin::absolute::LockTime::from_height(100).unwrap()),
                token_identifier: *TOKEN_IDENTIFIER,
                token_amount,
                is_frozen: Some(false),
                withdraw_height: None,
                withdraw_txid: None,
                withdraw_tx_vout: None,
                withdraw_block_hash: None,
            }],
            spark_operator_identity_public_keys: vec![*SO_PUBKEY],
            expiry_time: 0,
            network: Some(2),
            client_created_timestamp: 1000,
        };

        let partial_spark_hash = SparkHash::hash_token_transaction(&token_tx, true)?;
        let final_spark_hash = SparkHash::hash_token_transaction(&token_tx, false)?;

        assert_eq!(
            partial_spark_hash.0,
            Hash::from_str("fb0092e584b68bd27069e8f304118b2871c456d242382e29552e1ec985c1574a")
                .unwrap()
        );
        assert_eq!(
            final_spark_hash.0,
            Hash::from_str("8fe5fd5a5f109a3e3dc1517939a5dc4ab28f7fc11ec6c9e9319678e8e933aeba")
                .unwrap()
        );

        Ok(())
    }
}
