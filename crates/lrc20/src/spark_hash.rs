use std::ops::Deref;

use bitcoin::hashes::{Hash, HashEngine, sha256::Hash as Sha256Hash};
use proto_hasher::{ProtoHasher, errors::ProtoHasherError};
use spark_protos::{
    reflect::{SparkProtoReflectError, ToDynamicMessage},
    spark_token::token_transaction::TokenInputs,
};
use thiserror::Error;
use tracing::debug;
use uuid::Uuid;

use crate::{
    marshal::marshal_token_transaction,
    token_leaf::{TokenLeafOutput, TokenLeafToSpend},
    token_transaction::{TokenTransaction, TokenTransactionInput, TokenTransactionVersion},
};

/// A hash of the LRC20 receipt data that uniquely identifies a receipt (coin).
///
/// Defined as: `PXH = hash(hash(Y) || UV)`, where `Y` - is token_amount (amount),
/// and `UV` - is token type (issuer public key).
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
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
    pub fn hash_token_transaction(token_tx: &TokenTransaction, is_partial_hash: bool) -> Result<Self, SparkHashError> {
        if token_tx.version == TokenTransactionVersion::V4 {
            return SparkHash::hash_token_transaction_v4(token_tx, is_partial_hash);
        }
        debug!(version = ?token_tx.version, is_partial_hash, leaves_count = token_tx.leaves_to_create.len(),
           "Starting token transaction hash calculation");

        let mut hash_engine = Sha256Hash::engine();

        let not_v1 = token_tx.version != TokenTransactionVersion::V1;

        if not_v1 {
            hash_engine.input(Sha256Hash::hash(&token_tx.version.bytes()).as_byte_array());
        }

        let input_type = match token_tx.input {
            TokenTransactionInput::Create(..) => 1u8,
            TokenTransactionInput::Mint(..) => 2u8,
            TokenTransactionInput::Transfer(..) => 3u8,
        };
        hash_engine.input(Sha256Hash::hash(&[0, 0, 0, input_type]).as_byte_array());

        debug!("Hash inputs");
        match &token_tx.input {
            TokenTransactionInput::Transfer(transfer_input) => {
                let inputs_len = transfer_input.leaves_to_spend.len() as u32;
                hash_engine.input(Sha256Hash::hash(&inputs_len.to_be_bytes()).as_byte_array());

                for leaf in &transfer_input.leaves_to_spend {
                    hash_engine.input(SparkHash::hash_token_leaf_to_spend(leaf).0.as_byte_array());
                }
            }
            TokenTransactionInput::Mint(mint_input) => {
                hash_engine.input(Sha256Hash::hash(&mint_input.issuer_public_key.serialize()).as_byte_array());

                if let Some(identifier) = mint_input.token_identifier {
                    hash_engine.input(Sha256Hash::hash(&identifier.to_bytes()).as_byte_array());
                }
            }
            TokenTransactionInput::Create(_) => {
                return Err(SparkHashError::InvalidTokenTransactionInput);
            }
        }

        let outputs_len = token_tx.leaves_to_create.len() as u32;
        hash_engine.input(Sha256Hash::hash(&outputs_len.to_be_bytes()).as_byte_array());

        debug!("Hash output leaves");
        for leaf in &token_tx.leaves_to_create {
            hash_engine.input(
                SparkHash::hash_token_leaf_output(leaf, is_partial_hash)?
                    .0
                    .as_byte_array(),
            );
        }

        let mut so_pubkeys = token_tx.spark_operator_identity_public_keys.clone();
        so_pubkeys.sort();

        debug!("Hash spark operator identity public keys");
        let so_pubkeys_len = so_pubkeys.len() as u32;
        hash_engine.input(Sha256Hash::hash(&so_pubkeys_len.to_be_bytes()).as_byte_array());

        for key in &so_pubkeys {
            hash_engine.input(Sha256Hash::hash(&key.serialize()).as_byte_array());
        }

        debug!("Hash the network");
        if let Some(network) = token_tx.network {
            hash_engine.input(Sha256Hash::hash(&network.to_be_bytes()).as_byte_array());
        }

        if not_v1 {
            hash_engine.input(Sha256Hash::hash(&token_tx.client_created_timestamp.to_be_bytes()).as_byte_array());

            if !is_partial_hash {
                hash_engine.input(Sha256Hash::hash(&token_tx.expiry_time.to_be_bytes()).as_byte_array());
            }
        }

        if token_tx.version == TokenTransactionVersion::V3 {
            let mut attachments = token_tx.invoice_attachments.iter().collect::<Vec<(&Uuid, &String)>>();

            attachments.sort_by(|(l, _), (r, _)| l.as_bytes().cmp(r.as_bytes()));

            let attachments_len = attachments.len() as u32;
            hash_engine.input(Sha256Hash::hash(&attachments_len.to_be_bytes()).as_byte_array());

            for (_, invoice) in attachments.into_iter() {
                hash_engine.input(Sha256Hash::hash(invoice.as_bytes()).as_byte_array());
            }
        }

        debug!("Finished token transaction hash calculated successfully");
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
    pub fn hash_token_leaf_output(leaf: &TokenLeafOutput, is_partial_hash: bool) -> Result<Self, SparkHashError> {
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

    /// Hashes a token transaction V4 following the LRC20 specification.
    ///
    /// # Arguments
    ///
    /// * `token_tx` - The token transaction to hash.
    /// * `is_partial_hash` - Whether to hash the transaction partially.
    ///
    /// # Returns
    ///
    /// A `SparkHash` representing the hash of the token transaction.
    pub fn hash_token_transaction_v4(
        token_tx: &TokenTransaction,
        is_partial_hash: bool,
    ) -> Result<Self, SparkHashError> {
        let hasher = ProtoHasher::new();
        debug!(version = ?token_tx.version, is_partial_hash, "Starting V4 token transaction hash");
        let mut proto = marshal_token_transaction(token_tx, !is_partial_hash)
            .map_err(|err| SparkHashError::TokenTransactionMarshalError(err.to_string()))?;

        if is_partial_hash {
            proto.expiry_time = None;

            // safe unwrap
            match proto.token_inputs.as_mut().unwrap() {
                TokenInputs::CreateInput(input) => {
                    input.creation_entity_public_key = None;
                }
                TokenInputs::MintInput(_) | TokenInputs::TransferInput(_) => {
                    for output in &mut proto.token_outputs {
                        output.id = None;
                        output.revocation_commitment = None;
                        output.withdraw_bond_sats = None;
                        output.withdraw_relative_block_locktime = None;
                    }
                }
            }
        }

        let hash = hasher.hash_proto(proto.to_dynamic()?)?;
        debug!(version = ?token_tx.version, "V4 token transaction hash completed");
        Ok(Self(hash))
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

    /// Invalid token transaction
    #[error("Failed to convert token transaction to proto: {0}")]
    TokenTransactionMarshalError(String),

    /// Failed to hash proto
    #[error("Failed to hash proto: {0}")]
    ProtoHasherError(#[from] ProtoHasherError),

    /// Failed to convert proto into DynamicMessage
    #[error("Failed to convert proto into DynamicMessage: {0}")]
    ProtoReflectError(#[from] SparkProtoReflectError),
}

#[cfg(test)]
mod test {
    use core::str::FromStr;
    use std::collections::HashMap;

    use bitcoin::{Network, hashes::sha256::Hash as Sha256Hash, secp256k1};
    use once_cell::sync::Lazy;
    use token_identifier::TokenIdentifier;
    use uuid::Uuid;

    use super::SparkHash;
    use crate::{
        token_leaf::{TokenLeafOutput, TokenLeafToSpend},
        token_transaction::{
            TokenTransaction, TokenTransactionInput, TokenTransactionMintInput, TokenTransactionTransferInput,
            TokenTransactionVersion,
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
        secp256k1::PublicKey::from_slice(&[
            0x02, 242, 155, 208, 90, 72, 211, 120, 244, 69, 99, 28, 101, 149, 222, 123, 50, 252, 63, 99, 54, 137, 226,
            7, 224, 163, 122, 93, 248, 42, 159, 173, 45,
        ])
        .unwrap()
    });

    static IDENTITY_PUBKEY: Lazy<secp256k1::PublicKey> = Lazy::new(|| {
        secp256k1::PublicKey::from_slice(&[
            0x02, 25, 155, 208, 90, 72, 211, 120, 244, 69, 99, 28, 101, 149, 222, 123, 50, 252, 63, 99, 54, 137, 226,
            7, 224, 163, 122, 93, 248, 42, 159, 173, 46,
        ])
        .unwrap()
    });

    static REVOCATION_COMMITMENT: Lazy<secp256k1::PublicKey> = Lazy::new(|| {
        secp256k1::PublicKey::from_slice(&[
            0x02, 100, 155, 208, 90, 72, 211, 120, 244, 69, 99, 28, 101, 149, 222, 123, 50, 252, 63, 99, 54, 137, 226,
            7, 224, 163, 122, 93, 248, 42, 159, 173, 46,
        ])
        .unwrap()
    });

    static SO_PUBKEY: Lazy<secp256k1::PublicKey> = Lazy::new(|| {
        secp256k1::PublicKey::from_slice(&[
            0x02, 200, 155, 208, 90, 72, 211, 120, 244, 69, 99, 28, 101, 149, 222, 123, 50, 252, 63, 99, 54, 137, 226,
            7, 224, 163, 122, 93, 248, 42, 159, 173, 46,
        ])
        .unwrap()
    });

    static TEST_INVOICE_ATTACHMENTS: Lazy<HashMap<Uuid, String>> = Lazy::new(|| {
        HashMap::from([
            (Uuid::from_str("01988b9f-5276-7eaa-a4a7-b4b7cf2acdf8").unwrap(), "sprt1pgssypkrjhrpzt2hw0ggrmndanmm035ley75nxu3gejaju4wx9nq86lwzfjqsqgjzqqe3zul2fm8a24y576t0ne2ehup5fg2yz4r6hxlhatyu9kpw09s2fk36ta5j0k85qascf6snpuy4sp0rp4ezyspvs4qgmt9d4hnyggzqmpet3s394th85ypaek7eaahc60uj02fnwg5vewew2hrzesra0hqflc0vn".to_string()),
            (Uuid::from_str("01988b9f-c435-7aa9-8fb0-2cd830bc5988").unwrap(), "sprt1pgssypkrjhrpzt2hw0ggrmndanmm035ley75nxu3gejaju4wx9nq86lwzf5ssqgjzqqe3zulcs6h42v0kqkdsv9utxyp5fs2yz4r6hxlhatyu9kpw09s2fk36ta5j0k85qascf6snpuy4sp0rp4ezyszq86z5zryd9nxvmt9d4hnyggzqmpet3s394th85ypaek7eaahc60uj02fnwg5vewew2hrzesra0hql7r5ne".to_string())
        ])
    });

    use bitcoin::hashes::Hash;

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
            client_created_timestamp: 100,
            invoice_attachments: Default::default(),
        };

        let partial_spark_hash = SparkHash::hash_token_transaction(&token_tx, true)?;
        let final_spark_hash = SparkHash::hash_token_transaction(&token_tx, false)?;

        assert_eq!(
            partial_spark_hash.0,
            Sha256Hash::from_slice(&[
                0x3c, 0xd1, 0xfd, 0xe3, 0x66, 0x2d, 0x05, 0x47, 0x8d, 0x99, 0x75, 0xf3, 0x64, 0x23, 0x96, 0x78, 0x84,
                0x2f, 0xf7, 0xe4, 0x8f, 0x1a, 0xcc, 0xd2, 0x84, 0x87, 0x94, 0xe6, 0x71, 0x9d, 0x87, 0xbd,
            ])
            .unwrap()
        );
        assert_eq!(
            final_spark_hash.0,
            Sha256Hash::from_slice(&[
                0x2d, 0x27, 0xc5, 0xdd, 0x8b, 0x93, 0x6f, 0xcb, 0x3b, 0xb0, 0x3e, 0x97, 0xbe, 0x49, 0x10, 0xd8, 0xcf,
                0xb7, 0x43, 0x78, 0x50, 0x5c, 0xa2, 0xb7, 0x8e, 0x77, 0xc7, 0x11, 0xb4, 0x4a, 0x0d, 0x2b,
            ])
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
                    parent_leaf_hash: Sha256Hash::hash("previous transaction".as_bytes()),
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
            client_created_timestamp: 100,
            invoice_attachments: Default::default(),
        };

        let partial_spark_hash = SparkHash::hash_token_transaction(&token_tx, true)?;
        let final_spark_hash = SparkHash::hash_token_transaction(&token_tx, false)?;

        assert_eq!(
            partial_spark_hash.0,
            Sha256Hash::from_slice(&[
                0xe8, 0x8d, 0x13, 0xc2, 0x37, 0x00, 0xd2, 0x46, 0x26, 0xed, 0x62, 0x14, 0xf3, 0x04, 0x51, 0x85, 0xde,
                0x8a, 0x98, 0xcf, 0x51, 0x2c, 0x0d, 0xbc, 0x6a, 0x27, 0x9f, 0xc0, 0xbb, 0x7b, 0x56, 0x2a,
            ])
            .unwrap()
        );
        assert_eq!(
            final_spark_hash.0,
            Sha256Hash::from_slice(&[
                0x8f, 0xf2, 0xa8, 0xfa, 0xe0, 0x5d, 0xf8, 0xdc, 0xe1, 0x17, 0x0f, 0x25, 0xb1, 0x8a, 0x43, 0x64, 0x86,
                0x65, 0x8b, 0x4d, 0xf0, 0x4c, 0x2c, 0xa3, 0x35, 0xb1, 0xfa, 0x31, 0x53, 0x86, 0x81, 0xad,
            ])
            .unwrap()
        );

        Ok(())
    }

    #[test]
    fn test_mint_token_tx_hash_v3() -> Result<(), Box<dyn std::error::Error>> {
        let token_amount = 1000;

        let token_tx = TokenTransaction {
            version: TokenTransactionVersion::V3,
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
            client_created_timestamp: 100,
            invoice_attachments: TEST_INVOICE_ATTACHMENTS.clone(),
        };

        let partial_spark_hash = SparkHash::hash_token_transaction(&token_tx, true)?;
        let final_spark_hash = SparkHash::hash_token_transaction(&token_tx, false)?;

        assert_eq!(
            partial_spark_hash.0,
            Sha256Hash::from_slice(&[
                0xa2, 0x85, 0x55, 0x31, 0xd2, 0x4c, 0x96, 0x3e, 0x69, 0xc1, 0xc1, 0x66, 0x7a, 0x30, 0xdf, 0xe0, 0x3c,
                0x5f, 0xa4, 0xd2, 0x1, 0xa5, 0xeb, 0xea, 0x52, 0x17, 0xc3, 0xc9, 0x89, 0xac, 0x6b, 0xd
            ])
            .unwrap()
        );

        assert_eq!(
            final_spark_hash.0,
            Sha256Hash::from_slice(&[
                0xc4, 0xf4, 0x5f, 0x17, 0x8d, 0xaf, 0xdc, 0x4, 0xf1, 0xc7, 0x19, 0x1, 0x17, 0x80, 0xc4, 0xd, 0xb3,
                0x3e, 0x1f, 0xd8, 0x4f, 0x64, 0x35, 0x91, 0x6f, 0xae, 0x6c, 0x95, 0x5e, 0xee, 0x4d, 0x75
            ])
            .unwrap()
        );

        Ok(())
    }

    #[test]
    fn test_transfer_token_tx_hash_v3() -> Result<(), Box<dyn std::error::Error>> {
        let token_amount = 1000;

        let token_tx = TokenTransaction {
            version: TokenTransactionVersion::V3,
            input: TokenTransactionInput::Transfer(TokenTransactionTransferInput {
                leaves_to_spend: vec![TokenLeafToSpend {
                    parent_leaf_hash: Sha256Hash::hash("previous transaction".as_bytes()),
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
            client_created_timestamp: 100,
            invoice_attachments: TEST_INVOICE_ATTACHMENTS.clone(),
        };

        let partial_spark_hash = SparkHash::hash_token_transaction(&token_tx, true)?;
        let final_spark_hash = SparkHash::hash_token_transaction(&token_tx, false)?;

        assert_eq!(
            partial_spark_hash.0,
            Sha256Hash::from_slice(&[
                0x27, 0x68, 0xbc, 0x5c, 0xb4, 0xef, 0x22, 0xd3, 0x68, 0x34, 0xb3, 0x7e, 0xb9, 0xb4, 0xe4, 0x43, 0xec,
                0xf7, 0xb2, 0x50, 0x15, 0x6c, 0xd3, 0xa7, 0x9b, 0xb6, 0xb9, 0x70, 0xd0, 0xf3, 0x66, 0x5b
            ])
            .unwrap()
        );

        assert_eq!(
            final_spark_hash.0,
            Sha256Hash::from_slice(&[
                0x79, 0x32, 0x8f, 0xdc, 0xc7, 0x84, 0xac, 0xcf, 0x3f, 0xb8, 0x8d, 0x9c, 0xf9, 0x6e, 0x92, 0xfa, 0x6d,
                0xd4, 0x55, 0xe3, 0x7d, 0xfc, 0x52, 0xac, 0x4d, 0x4a, 0xb, 0x9f, 0xf0, 0xc2, 0xc7, 0x81
            ])
            .unwrap()
        );

        Ok(())
    }

    #[test]
    fn test_mint_token_tx_v4() -> Result<(), Box<dyn std::error::Error>> {
        let token_amount = 1000;

        let token_tx = TokenTransaction {
            version: TokenTransactionVersion::V4,
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
            expiry_time: 1,
            network: Some(2),
            client_created_timestamp: 100,
            invoice_attachments: TEST_INVOICE_ATTACHMENTS.clone(),
        };

        let partial_hash = SparkHash::hash_token_transaction(&token_tx, true)?;

        assert_eq!(
            "3594cc7673c2339bb097b4236b51d267717694294391fee92e99a3a361cb0ac4",
            partial_hash.0.to_string()
        );

        let final_hash = SparkHash::hash_token_transaction(&token_tx, false)?;
        assert_eq!(
            "8fe449eb9baa0e4642d327a6a5ef5d54910a93fdd7ef15a98979539575a5d781",
            final_hash.0.to_string()
        );

        Ok(())
    }
}
