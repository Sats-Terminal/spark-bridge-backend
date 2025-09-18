use crate::errors::SparkServiceError;
use bitcoin::secp256k1::PublicKey;
use chrono;
use frost::types::SigningMetadata;
use frost::types::TokenTransactionMetadata;
use lrc20::token_leaf::TokenLeafOutput;
use lrc20::token_transaction::TokenTransaction;
use lrc20::token_transaction::TokenTransactionCreateInput;
use lrc20::token_transaction::TokenTransactionInput;
use lrc20::token_transaction::TokenTransactionMintInput;
use lrc20::token_transaction::TokenTransactionVersion;
use spark_address::Network;
use token_identifier::TokenIdentifier;
use spark_protos::spark_token::TokenTransaction as TokenTransactionSparkProto;
use lrc20::marshal::marshal_token_transaction;

const DEFAULT_MAX_SUPPLY: u64 = 21_000_000_000;
const DEFAULT_DECIMALS: u32 = 8;
const DEFAULT_IS_FREEZABLE: bool = false;

#[derive(Debug, Clone)]
pub enum SparkTransactionType {
    Mint {
        receiver_identity_public_key: PublicKey,
        token_amount: u64,
    },
    Create {
        token_name: String,
        token_ticker: String,
    },
}

pub fn create_partial_token_transaction(
    issuer_public_key: PublicKey,
    spark_transaction_type: SparkTransactionType,
    token_identifier: TokenIdentifier,
    spark_operator_identity_public_keys: Vec<PublicKey>,
    network: Network,
) -> Result<TokenTransaction, SparkServiceError> {
    match spark_transaction_type {
        SparkTransactionType::Mint {
            receiver_identity_public_key,
            token_amount,
        } => {
            let token_transaction = TokenTransaction {
                version: TokenTransactionVersion::V2,
                input: TokenTransactionInput::Mint(TokenTransactionMintInput {
                    issuer_public_key: issuer_public_key,
                    token_identifier: Some(token_identifier),
                    issuer_signature: None,
                    issuer_provided_timestamp: None,
                }),
                leaves_to_create: vec![create_partial_token_leaf_output(
                    issuer_public_key,
                    receiver_identity_public_key,
                    token_identifier,
                    token_amount as u128,
                )],
                spark_operator_identity_public_keys,
                expiry_time: 0,
                network: Some(network as u32),
                client_created_timestamp: chrono::Utc::now().timestamp_millis() as u64,
                invoice_attachments: Default::default(),
            };
            Ok(token_transaction)
        }
        SparkTransactionType::Create {
            token_name,
            token_ticker,
        } => {
            let token_transaction = TokenTransaction {
                version: TokenTransactionVersion::V2,
                input: TokenTransactionInput::Create(TokenTransactionCreateInput {
                    issuer_public_key,
                    token_name,
                    token_ticker,
                    decimals: DEFAULT_DECIMALS,
                    max_supply: DEFAULT_MAX_SUPPLY as u128,
                    is_freezable: DEFAULT_IS_FREEZABLE,
                    creation_entity_public_key: None,
                }),
                leaves_to_create: vec![],
                spark_operator_identity_public_keys,
                expiry_time: 0,
                network: Some(network as u32),
                client_created_timestamp: chrono::Utc::now().timestamp_millis() as u64,
                invoice_attachments: Default::default(),
            };
            Ok(token_transaction)
        }
    }
}

fn create_partial_token_leaf_output(
    sender_identity_public_key: PublicKey,
    receiver_identity_public_key: PublicKey,
    token_identifier: TokenIdentifier,
    token_amount: u128,
) -> TokenLeafOutput {
    TokenLeafOutput {
        owner_public_key: sender_identity_public_key,
        revocation_public_key: receiver_identity_public_key,
        token_amount,
        token_identifier,
        is_frozen: None,
        withdraw_txid: None,
        withdraw_tx_vout: None,
        withdraw_height: None,
        withdraw_block_hash: None,
        id: None,
        withdrawal_bond_sats: None,
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
    let token_transaction_metadata: TokenTransactionMetadata = match (spark_transaction_type, is_partial) {
        (SparkTransactionType::Mint { .. }, true) => TokenTransactionMetadata::PartialMintToken { token_transaction: token_transaction_proto },
        (SparkTransactionType::Mint { .. }, false) => TokenTransactionMetadata::FinalMintToken { token_transaction: token_transaction_proto },
        (SparkTransactionType::Create { .. }, true) => {
            TokenTransactionMetadata::PartialCreateToken { token_transaction: token_transaction_proto }
        }
        (SparkTransactionType::Create { .. }, false) => {
            TokenTransactionMetadata::FinalCreateToken { token_transaction: token_transaction_proto }
        }
    };
    Ok(SigningMetadata {
        token_transaction_metadata,
    })
}
