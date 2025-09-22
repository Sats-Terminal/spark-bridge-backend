use crate::{
    token_leaf::{TokenLeafOutput, TokenLeafToSpend},
    token_transaction::{
        TokenTransaction, TokenTransactionCreateInput, TokenTransactionError, TokenTransactionInput,
        TokenTransactionMintInput, TokenTransactionTransferInput, TokenTransactionVersion,
    },
};
use bitcoin::{
    absolute::LockTime,
    hashes::{Hash, sha256::Hash as Sha256Hash},
    secp256k1::PublicKey,
};
use prost_wkt_types::Timestamp;
use spark_address::decode_spark_address;
use spark_protos::spark_token::{self, InvoiceAttachment, TokenTransaction as TokenTransactionV2SparkProto};
use std::collections::HashMap;
use token_identifier::TokenIdentifier;
use tracing::{debug, error};
use uuid::Uuid;

/// Converts a `TokenTransaction` struct into a `spark_protos::spark_token::TokenTransaction` message.
///
/// This function takes a `TokenTransaction` struct and converts its components into the corresponding Spark protocol message format.
///
/// # Arguments
///
/// * `tx` - The `TokenTransaction` struct to convert.
///
/// # Returns
///
/// A `spark_protos::spark::TokenTransaction` message.
pub fn marshal_token_transaction(
    tx: &TokenTransaction,
    with_revocation_commitments: bool,
) -> Result<TokenTransactionV2SparkProto, TokenTransactionError> {
    let spark_operator_identity_public_keys = tx
        .spark_operator_identity_public_keys
        .iter()
        .cloned()
        .map(|pubkey| pubkey.serialize().to_vec())
        .collect();

    let network = tx.network.ok_or(TokenTransactionError::NetworkMissing)?;

    // Assume that tx version is always v2

    match tx.version {
        TokenTransactionVersion::V2 => {
            let token_outputs =
                into_token_leaves_to_create_v2(tx.leaves_to_create.clone(), with_revocation_commitments)?;
            let token_inputs = Some(into_token_input_v2(tx.clone())?);

            let client_created_ts = Timestamp {
                seconds: tx.client_created_timestamp as i64 / 1_000,
                nanos: ((tx.client_created_timestamp % 1_000) * 1_000_000) as i32,
            };

            let expiry_time = (tx.expiry_time > 0).then(|| Timestamp {
                seconds: tx.expiry_time as i64,
                nanos: 0,
            });

            let tx_proto = TokenTransactionV2SparkProto {
                version: 1,
                token_outputs,
                spark_operator_identity_public_keys,
                expiry_time,
                network: network as i32,
                client_created_timestamp: Some(client_created_ts),
                token_inputs,
                invoice_attachments: Default::default(),
            };

            Ok(tx_proto)
        }
        TokenTransactionVersion::V3 | TokenTransactionVersion::V4 => {
            let token_outputs =
                into_token_leaves_to_create_v2(tx.leaves_to_create.clone(), with_revocation_commitments)?;
            let token_inputs = Some(into_token_input_v2(tx.clone())?);

            let client_created_ts = Timestamp {
                seconds: tx.client_created_timestamp as i64 / 1_000,
                nanos: ((tx.client_created_timestamp % 1_000) * 1_000_000) as i32,
            };

            let expiry_time = (tx.expiry_time > 0).then(|| Timestamp {
                seconds: tx.expiry_time as i64,
                nanos: 0,
            });

            let mut invoice_attachments: Vec<_> = tx
                .invoice_attachments
                .values()
                .into_iter()
                .map(|v| InvoiceAttachment {
                    spark_invoice: v.to_string(),
                })
                .collect();

            invoice_attachments.sort_by(|a, b| a.spark_invoice.cmp(&b.spark_invoice));

            let tx_proto = TokenTransactionV2SparkProto {
                version: tx.version.as_u32(),
                token_outputs,
                spark_operator_identity_public_keys,
                expiry_time,
                network: network as i32,
                client_created_timestamp: Some(client_created_ts),
                token_inputs,
                invoice_attachments,
            };

            Ok(tx_proto)
        }
        _ => {
            error!("Invalid token transaction version. {:?}", tx.version);
            let version_num = match tx.version {
                TokenTransactionVersion::V1 => 0u32,
                TokenTransactionVersion::V2 => 1u32,
                TokenTransactionVersion::V3 => 2u32,
                TokenTransactionVersion::V4 => 3u32,
            };
            return Err(TokenTransactionError::InvalidTokenTransactionVersion(version_num));
        }
    }
}

fn into_token_leaves_to_create_v2(
    leaves: Vec<TokenLeafOutput>,
    with_revocation_commitments: bool,
) -> Result<Vec<spark_protos::spark_token::TokenOutput>, TokenTransactionError> {
    let mut result_leaves = Vec::new();

    for leaf in leaves {
        // Do NOT set `revocation_commitment` when constructing the partial
        // token transaction. The Spark Operator will populate this field
        // during transaction finalisation. Supplying it here causes the
        // Spark backend to reject the request with
        // "output <n> revocation commitment will be added by the SO - do not set this field when starting transactions".
        let revocation_commitment =
            with_revocation_commitments.then(|| leaf.revocation_public_key.serialize().to_vec());

        let leaf_to_create = spark_protos::spark_token::TokenOutput {
            id: leaf.id,
            owner_public_key: leaf.owner_public_key.serialize().to_vec(),
            revocation_commitment,
            withdraw_bond_sats: leaf.withdrawal_bond_sats,
            withdraw_relative_block_locktime: leaf
                .withdrawal_locktime
                .map(|locktime| locktime.to_consensus_u32() as u64),
            token_public_key: None,
            token_identifier: Some(leaf.token_identifier.to_bytes().to_vec()),
            token_amount: leaf.token_amount.to_be_bytes().to_vec(),
        };

        result_leaves.push(leaf_to_create);
    }

    Ok(result_leaves)
}

/// Parses a token transaction from a Spark protocol message.
///
/// This function takes a `spark_protos::spark::TokenTransaction` and parses it.
/// Signature data (previously passed in) seems to be handled elsewhere now.
///
/// # Arguments
///
/// * `token_tx` - The token transaction to parse.
///
/// # Returns
///
/// A `TokenTransaction` struct.
pub fn unmarshal_token_transaction(
    token_tx: TokenTransactionV2SparkProto,
) -> Result<TokenTransaction, TokenTransactionError> {
    debug!("Unmarshalling token transaction: {:?}", token_tx);
    let token_input = token_tx.token_inputs.ok_or(TokenTransactionError::TokenInputMissing)?;

    let parsed_token_input = parse_token_input_v2(token_input)?;

    let leaves_to_create = parse_token_leaves_to_create_v2(token_tx.token_outputs)?;
    let mut spark_operator_identity_public_keys = Vec::new();
    for pubkey_bytes in token_tx.spark_operator_identity_public_keys {
        let pubkey = PublicKey::from_slice(&pubkey_bytes)?;
        spark_operator_identity_public_keys.push(pubkey);
    }

    let network = token_tx.network;
    let expiry_time = token_tx
        .expiry_time
        .map(|expiry_time| expiry_time.seconds as u64)
        .unwrap_or(0);
    let client_created_timestamp = token_tx
        .client_created_timestamp
        .map(|ts| ts.seconds as u64 * 1_000 + ts.nanos as u64 / 1_000_000)
        .unwrap_or(0);

    let version = match token_tx.version {
        0 => TokenTransactionVersion::V1,
        1 => TokenTransactionVersion::V2,
        2 => TokenTransactionVersion::V3,
        _ => {
            return Err(TokenTransactionError::InvalidTokenTransactionVersion(token_tx.version));
        }
    };

    let invoice_attachments: HashMap<Uuid, String> = token_tx
        .invoice_attachments
        .into_iter()
        .map(|attachment| {
            let encoded_spark_invoice = attachment.spark_invoice;
            let decoded_data = decode_spark_address(&encoded_spark_invoice)?;

            let invoice_fields = decoded_data.invoice.ok_or(TokenTransactionError::InvoiceDataMissing)?;

            Ok((invoice_fields.id, encoded_spark_invoice))
        })
        .collect::<Result<_, TokenTransactionError>>()?;

    Ok(TokenTransaction {
        version,
        input: parsed_token_input,
        leaves_to_create,
        spark_operator_identity_public_keys,
        network: Some(network as u32),
        expiry_time,
        client_created_timestamp,
        invoice_attachments,
    })
}

fn into_token_input_v2(
    tx: TokenTransaction,
) -> Result<spark_protos::spark_token::token_transaction::TokenInputs, TokenTransactionError> {
    let input = match tx.input {
        TokenTransactionInput::Mint(mint_input) => {
            let token_identifier = mint_input
                .token_identifier
                .ok_or(TokenTransactionError::TokenIdentifierMissing)?;
            spark_protos::spark_token::token_transaction::TokenInputs::MintInput(
                spark_protos::spark_token::TokenMintInput {
                    issuer_public_key: mint_input.issuer_public_key.serialize().to_vec(),
                    token_identifier: Some(token_identifier.to_bytes().to_vec()),
                },
            )
        }
        TokenTransactionInput::Transfer(transfer_input) => {
            let outputs_to_spend = into_token_leaves_to_spend_v2(transfer_input.leaves_to_spend.clone())?;
            spark_protos::spark_token::token_transaction::TokenInputs::TransferInput(
                spark_protos::spark_token::TokenTransferInput { outputs_to_spend },
            )
        }
        _ => {
            return Err(TokenTransactionError::InvalidTokenTransactionInput(format!(
                "{:?} is not allowed for token transactions V2",
                tx.input
            )));
        }
    };

    Ok(input)
}

fn into_token_leaves_to_spend_v2(
    leaves: Vec<TokenLeafToSpend>,
) -> Result<Vec<spark_protos::spark_token::TokenOutputToSpend>, TokenTransactionError> {
    let mut result_leaves = Vec::new();

    for leaf in leaves {
        let leaf_to_spend = spark_protos::spark_token::TokenOutputToSpend {
            prev_token_transaction_hash: leaf.parent_leaf_hash.to_byte_array().to_vec(),
            prev_token_transaction_vout: leaf.parent_leaf_index,
        };

        result_leaves.push(leaf_to_spend);
    }

    Ok(result_leaves)
}

fn parse_token_input_v2(
    token_input: spark_token::token_transaction::TokenInputs,
) -> Result<TokenTransactionInput, TokenTransactionError> {
    let parsed_token_input = match token_input {
        spark_token::token_transaction::TokenInputs::MintInput(issue_input) => {
            let issuer_public_key = PublicKey::from_slice(&issue_input.issuer_public_key)?;
            let token_identifier = issue_input
                .token_identifier
                .map(|token_identifier_bytes| TokenIdentifier::from_bytes(&token_identifier_bytes))
                .transpose()?;

            TokenTransactionInput::Mint(TokenTransactionMintInput {
                issuer_public_key,
                token_identifier,
                issuer_signature: None,
                issuer_provided_timestamp: None,
            })
        }
        spark_token::token_transaction::TokenInputs::TransferInput(transfer_input) => {
            let leaves_to_spend = parse_token_leaves_to_spend_v2(transfer_input.outputs_to_spend)?;
            TokenTransactionInput::Transfer(TokenTransactionTransferInput { leaves_to_spend })
        }
        spark_token::token_transaction::TokenInputs::CreateInput(create_input) => {
            let issuer_public_key = PublicKey::from_slice(&create_input.issuer_public_key)?;
            let creation_entity_public_key = create_input
                .creation_entity_public_key
                .map(|public_key_bytes| PublicKey::from_slice(&public_key_bytes))
                .transpose()?;
            let max_supply_bytes: [u8; 16] = create_input.max_supply.as_slice().try_into()?;
            let max_supply = u128::from_be_bytes(max_supply_bytes);

            TokenTransactionInput::Create(TokenTransactionCreateInput {
                issuer_public_key,
                token_name: create_input.token_name,
                token_ticker: create_input.token_ticker,
                decimals: create_input.decimals,
                max_supply,
                is_freezable: create_input.is_freezable,
                creation_entity_public_key,
            })
        }
    };

    Ok(parsed_token_input)
}

fn parse_token_leaves_to_spend_v2(
    leaves: Vec<spark_protos::spark_token::TokenOutputToSpend>,
) -> Result<Vec<TokenLeafToSpend>, TokenTransactionError> {
    let mut result_leaves = Vec::new();

    for leaf in leaves {
        let parent_leaf_hash = Sha256Hash::from_slice(&leaf.prev_token_transaction_hash)?;
        let parent_leaf_index = leaf.prev_token_transaction_vout;

        let leaf_to_spend = TokenLeafToSpend {
            parent_leaf_hash,
            parent_leaf_index,
        };

        result_leaves.push(leaf_to_spend);
    }

    Ok(result_leaves)
}

fn parse_token_leaves_to_create_v2(
    leaves: Vec<spark_protos::spark_token::TokenOutput>,
) -> Result<Vec<TokenLeafOutput>, TokenTransactionError> {
    let mut result_leaves = Vec::new();

    for leaf in leaves {
        debug!("Parsing token leaf: {:?}", leaf);
        let id = leaf.id;
        let owner_public_key = PublicKey::from_slice(&leaf.owner_public_key)?;
        let revocation_public_key = PublicKey::from_slice(
            &leaf
                .revocation_commitment
                .as_ref()
                .ok_or(TokenTransactionError::RevocationPublicKeyMissing)?,
        )?;
        let withdrawal_bond_sats = leaf.withdraw_bond_sats;
        let withdrawal_locktime = leaf
            .withdraw_relative_block_locktime
            .map(|locktime| LockTime::from_consensus(locktime as u32));
        let token_amount_bytes: [u8; 16] = leaf.token_amount.as_slice().try_into()?;
        let token_amount = u128::from_be_bytes(token_amount_bytes);
        let token_identifier = TokenIdentifier::from_bytes(
            &leaf
                .token_identifier
                .ok_or(TokenTransactionError::TokenIdentifierMissing)?,
        )?;

        let leaf_to_create = TokenLeafOutput {
            id,
            owner_public_key,
            revocation_public_key,
            withdrawal_bond_sats,
            withdrawal_locktime,
            token_amount,
            token_identifier,
            is_frozen: None,
            withdraw_txid: None,
            withdraw_tx_vout: None,
            withdraw_height: None,
            withdraw_block_hash: None,
        };

        result_leaves.push(leaf_to_create);
    }

    Ok(result_leaves)
}
