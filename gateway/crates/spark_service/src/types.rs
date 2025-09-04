use lrc20::token_transaction::TokenTransaction;
use lrc20::token_transaction::TokenTransactionVersion;
use lrc20::token_transaction::TokenTransactionTransferInput;
use lrc20::token_leaf::TokenLeafToSpend;
use lrc20::token_transaction::TokenTransactionInput;
use lrc20::token_transaction::TokenTransactionCreateInput;
use lrc20::token_transaction::TokenTransactionMintInput;
use lrc20::token_leaf::TokenLeafOutput;
use lrc20::token_identifier::TokenIdentifier;
use crate::errors::SparkServiceError;
use spark_client::utils::spark_address::Network;
use bitcoin::secp256k1::PublicKey;
use chrono;

const DEFAULT_MAX_SUPPLY: u128 = 21_000_000_000;
const DEFAULT_DECIMALS: u32 = 8;
const DEFAULT_IS_FREEZABLE: bool = false;

#[derive(Debug, Clone)]
pub enum SparkTransactionType {
    Mint {
        sender_identity_public_key: PublicKey,
        receiver_identity_public_key: PublicKey,
        token_amount: u128,
    },
    Transfer {
        leaves_to_spend: Vec<TokenLeafToSpend>,
        leaves_to_spend_token_outputs: Vec<TokenLeafOutput>,
        sender_identity_public_key: PublicKey,
        receiver_identity_public_key: PublicKey,
        token_amount: u128,
    },
    Create {
        sender_identity_public_key: PublicKey,
        token_name: String,
        token_ticker: String,
    },
}

#[derive(Debug, Clone)]
pub struct SparkTransactionRequest {
    token_identifier: TokenIdentifier,
    transaction_type: SparkTransactionType,
    network: Network,
    spark_operator_identity_public_keys: Vec<PublicKey>,
}

pub fn create_partial_token_transaction(request: SparkTransactionRequest) -> Result<TokenTransaction, SparkServiceError> {
    match request.transaction_type {
        SparkTransactionType::Mint {
            sender_identity_public_key,
            receiver_identity_public_key,
            token_amount,
        } => {
            let token_transaction = TokenTransaction {
                version: TokenTransactionVersion::V2,
                input: TokenTransactionInput::Mint(TokenTransactionMintInput {
                    issuer_public_key: sender_identity_public_key,
                    token_identifier: Some(request.token_identifier),
                    issuer_signature: None,
                    issuer_provided_timestamp: None,
                }),
                leaves_to_create: vec![
                    create_partial_token_leaf_output(
                        sender_identity_public_key, 
                        receiver_identity_public_key, 
                        request.token_identifier, 
                        token_amount
                    )
                ],
                spark_operator_identity_public_keys: request.spark_operator_identity_public_keys,
                expiry_time: 0,
                network: Some(request.network as u32),
                client_created_timestamp: chrono::Utc::now().timestamp_millis() as u64,
            };
            Ok(token_transaction)
        }
        SparkTransactionType::Transfer { 
            leaves_to_spend,
            leaves_to_spend_token_outputs,
            sender_identity_public_key,
            receiver_identity_public_key,
            token_amount,
        } => {
            let total_input_amount = leaves_to_spend_token_outputs.iter().map(|output| output.token_amount).sum::<u128>();
            if total_input_amount < token_amount {
                return Err(SparkServiceError::InvalidData("Total input amount is less than token amount".to_string()));
            }
            if leaves_to_spend.len() != leaves_to_spend_token_outputs.len() {
                return Err(SparkServiceError::InvalidData("Leaves to spend and leaves to spend token outputs length mismatch".to_string()));
            }
            for token_output in leaves_to_spend_token_outputs {
                if token_output.token_identifier != request.token_identifier {
                    return Err(SparkServiceError::InvalidData("Token identifier mismatch".to_string()));
                }
            }

            let token_transaction = TokenTransaction {
                version: TokenTransactionVersion::V2,
                input: TokenTransactionInput::Transfer(TokenTransactionTransferInput {
                    leaves_to_spend: leaves_to_spend,
                }),
                leaves_to_create: vec![
                    create_partial_token_leaf_output(
                        sender_identity_public_key, 
                        receiver_identity_public_key, 
                        request.token_identifier, 
                        token_amount
                    ),
                    create_partial_token_leaf_output(
                        sender_identity_public_key, 
                        sender_identity_public_key, 
                        request.token_identifier, 
                        total_input_amount - token_amount
                    )
                ],
                spark_operator_identity_public_keys: request.spark_operator_identity_public_keys,
                expiry_time: 0,
                network: Some(request.network as u32),
                client_created_timestamp: chrono::Utc::now().timestamp_millis() as u64,
            };
            Ok(token_transaction)
        }
        SparkTransactionType::Create {
            sender_identity_public_key,
            token_name,
            token_ticker,
        } => {
            let token_transaction = TokenTransaction {
                version: TokenTransactionVersion::V2,
                input: TokenTransactionInput::Create(TokenTransactionCreateInput {
                    issuer_public_key: sender_identity_public_key,
                    token_name: token_name,
                    token_ticker: token_ticker,
                    decimals: DEFAULT_DECIMALS,
                    max_supply: DEFAULT_MAX_SUPPLY,
                    is_freezable: DEFAULT_IS_FREEZABLE,
                    creation_entity_public_key: None,
                }),
                leaves_to_create: vec![
                    create_partial_token_leaf_output(
                        sender_identity_public_key, 
                        sender_identity_public_key, 
                        request.token_identifier, 
                        DEFAULT_MAX_SUPPLY
                    )
                ],
                spark_operator_identity_public_keys: request.spark_operator_identity_public_keys,
                expiry_time: 0,
                network: Some(request.network as u32),
                client_created_timestamp: chrono::Utc::now().timestamp_millis() as u64,
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
        token_amount: token_amount,
        token_identifier: token_identifier,
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