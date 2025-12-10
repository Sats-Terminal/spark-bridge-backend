use crate::errors::SparkServiceError;
use crate::utils::WRunesMetadata;
use bitcoin::secp256k1::PublicKey;
use chrono;
use frost::types::SigningMetadata;
use lrc20::marshal::marshal_token_transaction;
use lrc20::token_leaf::TokenLeafOutput;
use lrc20::token_leaf::TokenLeafToSpend;
use lrc20::token_metadata::DEFAULT_IS_FREEZABLE;
use lrc20::token_transaction::TokenTransaction;
use lrc20::token_transaction::TokenTransactionCreateInput;
use lrc20::token_transaction::TokenTransactionInput;
use lrc20::token_transaction::TokenTransactionMintInput;
use lrc20::token_transaction::TokenTransactionTransferInput;
use lrc20::token_transaction::TokenTransactionVersion;
use spark_address::decode_spark_address;
use std::str::FromStr;
use token_identifier::TokenIdentifier;

#[derive(Debug, Clone)]
pub enum SparkTransactionType {
    Mint {
        receiver_spark_address: String,
        token_amount: u64,
    },
    Create {
        wrunes_metadata: WRunesMetadata,
    },
    Transfer {
        sender_spark_address: String,
        receiver_spark_address: String,
        transfer_amount: u64,
        change_amount: u64,
        token_leaves_to_spend: Vec<TokenLeafToSpend>,
    },
}

pub fn create_partial_token_transaction(
    issuer_public_key: PublicKey,
    spark_transaction_type: SparkTransactionType,
    token_identifier: TokenIdentifier,
    spark_operator_identity_public_keys: Vec<PublicKey>,
    network: u32,
) -> Result<TokenTransaction, SparkServiceError> {
    match spark_transaction_type {
        SparkTransactionType::Mint {
            receiver_spark_address,
            token_amount,
        } => {
            let spark_address_data = decode_spark_address(&receiver_spark_address)?;
            let receiver_identity_public_key = PublicKey::from_str(&spark_address_data.identity_public_key)?;

            let token_transaction = TokenTransaction {
                version: TokenTransactionVersion::V4,
                input: TokenTransactionInput::Mint(TokenTransactionMintInput {
                    issuer_public_key,
                    token_identifier: Some(token_identifier),
                    issuer_signature: None,
                    issuer_provided_timestamp: None,
                }),
                leaves_to_create: vec![create_partial_token_leaf_output(
                    receiver_identity_public_key,
                    token_identifier,
                    token_amount as u128,
                )],
                spark_operator_identity_public_keys,
                expiry_time: 0,
                network: Some(network),
                client_created_timestamp: chrono::Utc::now().timestamp_millis() as u64,
                invoice_attachments: Default::default(),
            };
            Ok(token_transaction)
        }
        SparkTransactionType::Create { wrunes_metadata } => {
            let token_transaction = TokenTransaction {
                version: TokenTransactionVersion::V4,
                input: TokenTransactionInput::Create(TokenTransactionCreateInput {
                    issuer_public_key,
                    token_name: wrunes_metadata.token_name,
                    token_ticker: wrunes_metadata.token_ticker,
                    decimals: wrunes_metadata.decimals as u32,
                    max_supply: wrunes_metadata.max_supply,
                    is_freezable: DEFAULT_IS_FREEZABLE,
                    creation_entity_public_key: None,
                }),
                leaves_to_create: vec![],
                spark_operator_identity_public_keys,
                expiry_time: 0,
                network: Some(network),
                client_created_timestamp: chrono::Utc::now().timestamp_millis() as u64,
                invoice_attachments: Default::default(),
            };
            Ok(token_transaction)
        }
        SparkTransactionType::Transfer {
            sender_spark_address,
            receiver_spark_address,
            transfer_amount,
            change_amount,
            token_leaves_to_spend,
        } => {
            let receiver_spark_address_data = decode_spark_address(&receiver_spark_address)?;
            let receiver_identity_public_key = PublicKey::from_str(&receiver_spark_address_data.identity_public_key)?;

            let mut leaves_to_create = vec![create_partial_token_leaf_output(
                receiver_identity_public_key,
                token_identifier,
                transfer_amount as u128,
            )];
            if change_amount > 0 {
                let sender_spark_address_data = decode_spark_address(&sender_spark_address)?;
                let sender_identity_public_key = PublicKey::from_str(&sender_spark_address_data.identity_public_key)?;
                leaves_to_create.push(create_partial_token_leaf_output(
                    sender_identity_public_key,
                    token_identifier,
                    change_amount as u128,
                ));
            }

            let token_transaction = TokenTransaction {
                version: TokenTransactionVersion::V4,
                input: TokenTransactionInput::Transfer(TokenTransactionTransferInput {
                    leaves_to_spend: token_leaves_to_spend,
                }),
                leaves_to_create,
                spark_operator_identity_public_keys,
                expiry_time: 0,
                network: Some(network),
                client_created_timestamp: chrono::Utc::now().timestamp_millis() as u64,
                invoice_attachments: Default::default(),
            };
            Ok(token_transaction)
        }
    }
}

fn create_partial_token_leaf_output(
    receiver_identity_public_key: PublicKey,
    token_identifier: TokenIdentifier,
    token_amount: u128,
) -> TokenLeafOutput {
    // Spark mainnet currently enforces a fixed withdrawal bond; set it explicitly to satisfy operator validation.
    const DEFAULT_WITHDRAW_BOND_SATS: u64 = 10_000;
    TokenLeafOutput {
        owner_public_key: receiver_identity_public_key,
        revocation_public_key: receiver_identity_public_key,
        token_amount,
        token_identifier,
        is_frozen: None,
        withdraw_txid: None,
        withdraw_tx_vout: None,
        withdraw_height: None,
        withdraw_block_hash: None,
        id: None,
        withdrawal_bond_sats: Some(DEFAULT_WITHDRAW_BOND_SATS),
        withdrawal_locktime: None,
    }
}

pub fn create_signing_metadata(
    token_transaction: TokenTransaction,
    spark_transaction_type: SparkTransactionType,
    is_partial: bool,
) -> Result<SigningMetadata, SparkServiceError> {
    let token_transaction_proto = marshal_token_transaction(&token_transaction, is_partial)
        .map_err(|e| SparkServiceError::InvalidData(format!("Failed to marshal token transaction: {:?}", e)))?;
    let signing_metadata: SigningMetadata = match (spark_transaction_type, is_partial) {
        (SparkTransactionType::Mint { .. }, true) => SigningMetadata::PartialMintToken {
            token_transaction: token_transaction_proto,
        },
        (SparkTransactionType::Mint { .. }, false) => SigningMetadata::FinalMintToken {
            token_transaction: token_transaction_proto,
        },
        (SparkTransactionType::Create { .. }, true) => SigningMetadata::PartialCreateToken {
            token_transaction: token_transaction_proto,
        },
        (SparkTransactionType::Create { .. }, false) => SigningMetadata::FinalCreateToken {
            token_transaction: token_transaction_proto,
        },
        (SparkTransactionType::Transfer { .. }, true) => SigningMetadata::PartialTransferToken {
            token_transaction: token_transaction_proto,
        },
        (SparkTransactionType::Transfer { .. }, false) => SigningMetadata::FinalTransferToken {
            token_transaction: token_transaction_proto,
        },
    };
    Ok(signing_metadata)
}
