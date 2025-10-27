use crate::constants::{DEFAULT_DUST_AMOUNT, DEFAULT_FEE_AMOUNT, PAYING_INPUT_SATS_AMOUNT};
use crate::error::RuneError;
use crate::gateway_client::UserPayingTransferInput;
use crate::spark_client::GetSparkAddressDataRequest;
use crate::spark_client::SparkClient;
use crate::utils::create_credentials;
use crate::utils::sign_transaction;
use bitcoin::Address;
use bitcoin::Transaction;
use bitcoin::Txid;
use bitcoin::hashes::sha256::Hash as Sha256Hash;
use bitcoin::hashes::{Hash, HashEngine};
use bitcoin::key::TapTweak;
use bitcoin::secp256k1::Message;
use bitcoin::secp256k1::Secp256k1;
use bitcoin::secp256k1::{Keypair, PublicKey};
use bitcoin::sighash::{Prevouts, SighashCache, TapSighashType};
use bitcoin::transaction::Version;
use bitcoin::{Amount, OutPoint, ScriptBuf, Sequence, TxIn, TxOut, Witness};
use btc_indexer_client::client_api::AddrUtxoData;
use chrono::Utc;
use global_utils::common_types::get_uuid;
use global_utils::conversion::{bitcoin_network_to_proto_network, convert_network_to_spark_network};
use lrc20::marshal::marshal_token_transaction;
use lrc20::token_leaf::TokenLeafOutput;
use lrc20::token_leaf::TokenLeafToSpend;
use lrc20::token_transaction::TokenTransaction;
use lrc20::token_transaction::TokenTransactionInput;
use lrc20::token_transaction::TokenTransactionTransferInput;
use lrc20::token_transaction::TokenTransactionVersion;
use ordinals::RuneId;
use ordinals::{Edict, Runestone};
use proto_hasher::ProtoHasher;
use spark_address::{SparkAddressData, decode_spark_address, encode_spark_address};
use spark_protos::reflect::ToDynamicMessage;
use spark_protos::spark_token::CommitTransactionRequest;
use spark_protos::spark_token::InputTtxoSignaturesPerOperator;
use spark_protos::spark_token::SignatureWithIndex;
use spark_protos::spark_token::StartTransactionRequest;
use std::str::FromStr;
use token_identifier::TokenIdentifier;
use tracing;
use uuid::Uuid;

pub enum TransferType {
    RuneTransfer { rune_amount: u64 },
    BtcTransfer { sats_amount: u64 },
}

pub struct UserWallet {
    network: bitcoin::Network,
    p2tr_address: Address,
    keypair: Keypair,
    spark_client: SparkClient,
    rune_id: RuneId,
    proto_hasher: ProtoHasher,
    user_id: Uuid,
}

impl UserWallet {
    pub async fn new(
        spark_client: SparkClient,
        rune_id: RuneId,
        network: bitcoin::Network,
        key: Option<&str>,
    ) -> Result<Self, RuneError> {
        tracing::info!("Creating user wallet");
        let (p2tr_address, keypair) = create_credentials(network, key);

        Ok(Self {
            network: network,
            p2tr_address,
            keypair,
            spark_client,
            rune_id,
            proto_hasher: ProtoHasher::new(),
            user_id: get_uuid(),
        })
    }

    pub fn get_address(&self) -> Address {
        self.p2tr_address.clone()
    }

    pub fn get_user_id(&self) -> String {
        self.user_id.to_string()
    }

    pub fn get_spark_address(&self) -> Result<String, RuneError> {
        let identity_public_key = self.keypair.public_key();
        let spark_address = encode_spark_address(SparkAddressData {
            identity_public_key: identity_public_key.to_string(),
            network: convert_network_to_spark_network(self.network),
            invoice: None,
            signature: None,
        })?;
        Ok(spark_address)
    }

    pub async fn get_rune_balance(&self, address_rune_utxos: &[AddrUtxoData]) -> Result<u64, RuneError> {
        let mut total_balance = 0;
        for rune_utxos in address_rune_utxos.iter() {
            if !rune_utxos.spent {
                for runes in rune_utxos.runes.iter() {
                    if runes.rune_id.to_string() == self.rune_id.to_string() {
                        total_balance += runes.amount;
                    }
                }
            }
        }
        Ok(total_balance as u64)
    }

    pub async fn get_funded_outpoint_data(
        &self,
        address_rune_utxos: &[AddrUtxoData],
    ) -> Result<(OutPoint, u64), RuneError> {
        for rune_utxos in address_rune_utxos.iter() {
            if !rune_utxos.spent {
                for rune in rune_utxos.runes.iter() {
                    if rune.rune_id.to_string() == self.rune_id.to_string() {
                        if rune_utxos.value >= 100_000 && rune.amount >= 10_000 {
                            return Ok((
                                OutPoint {
                                    txid: Txid::from_str(&rune_utxos.txid).unwrap(),
                                    vout: rune_utxos.vout,
                                },
                                rune_utxos.value,
                            ));
                        }
                        tracing::warn!("There are funded runes, but the amount is less than 10_000");
                    }
                }
            }
        }
        Err(RuneError::GetFundedOutpointError(
            "Failed to get funded outpoint".to_string(),
        ))
    }

    pub async fn build_transfer_tx(
        &mut self,
        transfer_type: TransferType,
        transfer_address: Address,
        address_rune_utxos: &[AddrUtxoData],
    ) -> Result<Transaction, RuneError> {
        tracing::info!("Transferring runes");
        let rune_balance = self.get_rune_balance(address_rune_utxos).await?;

        let edicts = match transfer_type {
            TransferType::RuneTransfer { rune_amount } => {
                if rune_amount > rune_balance {
                    return Err(RuneError::InsufficientBalanceError("Insufficient balance".to_string()));
                }
                let mut edicts = vec![Edict {
                    id: self.rune_id,
                    amount: rune_amount as u128,
                    output: 1,
                }];
                if rune_amount < rune_balance {
                    edicts.push(Edict {
                        id: self.rune_id,
                        amount: (rune_balance - rune_amount) as u128,
                        output: 2,
                    });
                }
                edicts
            }
            TransferType::BtcTransfer { sats_amount: _ } => {
                vec![Edict {
                    id: self.rune_id,
                    amount: rune_balance as u128,
                    output: 2,
                }]
            }
        };

        let (outpoint, value) = self.get_funded_outpoint_data(address_rune_utxos).await?;

        let runestone = Runestone {
            edicts,
            etching: None,
            mint: None,
            pointer: None,
        };
        let op_return_script = runestone.encipher();

        let txin = TxIn {
            previous_output: outpoint,
            script_sig: ScriptBuf::new(),
            sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
            witness: Witness::new(),
        };

        let mut txouts = vec![TxOut {
            value: Amount::from_sat(0),
            script_pubkey: op_return_script,
        }];

        match transfer_type {
            TransferType::RuneTransfer { rune_amount: _ } => {
                txouts.extend(vec![
                    TxOut {
                        value: Amount::from_sat(DEFAULT_DUST_AMOUNT),
                        script_pubkey: transfer_address.script_pubkey(),
                    },
                    TxOut {
                        value: Amount::from_sat(value - DEFAULT_FEE_AMOUNT - DEFAULT_DUST_AMOUNT),
                        script_pubkey: self.p2tr_address.script_pubkey(),
                    },
                ]);
            }
            TransferType::BtcTransfer { sats_amount } => {
                txouts.extend(vec![
                    TxOut {
                        value: Amount::from_sat(sats_amount),
                        script_pubkey: transfer_address.script_pubkey(),
                    },
                    TxOut {
                        value: Amount::from_sat(value - DEFAULT_FEE_AMOUNT - sats_amount),
                        script_pubkey: self.p2tr_address.script_pubkey(),
                    },
                ]);
            }
        }

        let mut transaction = Transaction {
            version: Version::TWO,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vec![txin],
            output: txouts,
        };

        sign_transaction(&mut transaction, vec![value], self.p2tr_address.clone(), self.keypair)?;

        tracing::info!("Built transaction for rune transfer");

        Ok(transaction)
    }

    pub async fn build_unite_unspent_utxos_tx(
        &mut self,
        address_utxos: &[AddrUtxoData],
    ) -> Result<Transaction, RuneError> {
        let mut total_btc = 0;
        let mut total_runes = 0;
        let mut txins = vec![];
        let mut prev_input_amounts = vec![];

        for utxo_data in address_utxos.iter() {
            if !utxo_data.spent && utxo_data.value > 0 {
                total_btc += utxo_data.value;
                for runes in utxo_data.runes.iter() {
                    if runes.rune_id.to_string() == self.rune_id.to_string() {
                        total_runes += runes.amount;
                    }
                }
                txins.push(TxIn {
                    previous_output: OutPoint {
                        txid: Txid::from_str(&utxo_data.txid).unwrap(),
                        vout: utxo_data.vout,
                    },
                    script_sig: ScriptBuf::new(),
                    sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
                    witness: Witness::new(),
                });
                prev_input_amounts.push(utxo_data.value);
            }
        }

        let runestone = Runestone {
            edicts: vec![Edict {
                id: self.rune_id,
                amount: total_runes as u128,
                output: 1,
            }],
            etching: None,
            mint: None,
            pointer: None,
        };
        let op_return_script = runestone.encipher();

        let txouts = vec![
            TxOut {
                value: Amount::from_sat(0),
                script_pubkey: op_return_script,
            },
            TxOut {
                value: Amount::from_sat(total_btc - DEFAULT_FEE_AMOUNT),
                script_pubkey: self.p2tr_address.script_pubkey(),
            },
        ];

        let mut transaction = Transaction {
            version: Version::TWO,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: txins,
            output: txouts,
        };

        sign_transaction(
            &mut transaction,
            prev_input_amounts,
            self.p2tr_address.clone(),
            self.keypair,
        )?;

        Ok(transaction)
    }

    pub async fn transfer_spark(
        &mut self,
        transfer_amount: u64,
        receiver_spark_address: String,
    ) -> Result<(), RuneError> {
        tracing::info!("Transferring spark");

        let receiver_spark_address_data = decode_spark_address(&receiver_spark_address)?;
        let receiver_identity_public_key = PublicKey::from_str(&receiver_spark_address_data.identity_public_key)
            .map_err(|_| RuneError::InvalidData("Failed to decode receiver identity public key".to_string()))?;

        let spark_address_data = self
            .spark_client
            .get_spark_address_data(GetSparkAddressDataRequest {
                spark_address: self.get_spark_address()?,
            })
            .await?;

        tracing::info!("Spark address data, {:?}", spark_address_data);
        let token_identifier = spark_address_data.token_outputs[0].token_identifier;
        for token_output in spark_address_data.token_outputs.iter() {
            if token_output.token_identifier != token_identifier {
                return Err(RuneError::TokenIdentifierMismatch);
            }
        }

        let total_amount = spark_address_data
            .token_outputs
            .iter()
            .map(|token_output| token_output.amount)
            .sum::<u128>();
        let mut token_leaves_to_spend = vec![];
        for token_output in spark_address_data.token_outputs.iter() {
            token_leaves_to_spend.push(TokenLeafToSpend {
                parent_leaf_hash: *Sha256Hash::from_bytes_ref(
                    token_output
                        .prev_token_transaction_hash
                        .clone()
                        .as_slice()
                        .try_into()
                        .map_err(|_| {
                            RuneError::InvalidData(
                                "Failed to convert prev_token_transaction_hash to Sha256Hash".to_string(),
                            )
                        })?,
                ),
                parent_leaf_index: token_output.prev_token_transaction_vout,
            });
        }

        let mut leaves_to_create = vec![create_partial_token_leaf_output(
            receiver_identity_public_key,
            token_identifier,
            transfer_amount as u128,
        )];

        tracing::debug!("Token identifier: {:?}", token_identifier.encode_bech32m(self.network));
        tracing::debug!("Spark address: {:?}", receiver_spark_address);

        if (transfer_amount as u128) < total_amount {
            let changed_leaf_output = create_partial_token_leaf_output(
                self.keypair.public_key(),
                token_identifier,
                total_amount - transfer_amount as u128,
            );
            leaves_to_create.push(changed_leaf_output);
        }

        let partial_token_transaction = TokenTransaction {
            version: TokenTransactionVersion::V4,
            input: TokenTransactionInput::Transfer(TokenTransactionTransferInput {
                leaves_to_spend: token_leaves_to_spend,
            }),
            leaves_to_create,
            spark_operator_identity_public_keys: self.spark_client.get_operator_public_keys(),
            expiry_time: 0,
            network: Some(bitcoin_network_to_proto_network(self.network)),
            client_created_timestamp: Utc::now().timestamp_millis() as u64,
            invoice_attachments: Default::default(),
        };

        let partial_token_transaction_proto = marshal_token_transaction(&partial_token_transaction, false)
            .map_err(|e| RuneError::InvalidData(format!("Failed to marshal partial token transaction: {:?}", e)))?;

        let partial_token_transaction_hash = self
            .proto_hasher
            .hash_proto(
                partial_token_transaction_proto
                    .to_dynamic()
                    .map_err(|e| RuneError::HashError(format!("Failed to hash partial token transaction: {:?}", e)))?,
            )
            .map_err(|e| RuneError::HashError(format!("Failed to hash partial token transaction: {:?}", e)))?;

        let secp = Secp256k1::new();
        let message = Message::from_digest(partial_token_transaction_hash.to_byte_array());
        let signature = secp.sign_schnorr_no_aux_rand(&message, &self.keypair);

        let start_transaction_request = StartTransactionRequest {
            identity_public_key: self.keypair.public_key().serialize().to_vec(),
            partial_token_transaction: Some(partial_token_transaction_proto),
            partial_token_transaction_owner_signatures: vec![SignatureWithIndex {
                signature: signature.serialize().to_vec(),
                input_index: 0,
            }],
            validity_duration_seconds: 300,
        };
        let start_transaction_response = self
            .spark_client
            .start_spark_transaction(start_transaction_request, self.keypair)
            .await?;

        let final_token_transaction_proto = start_transaction_response
            .final_token_transaction
            .ok_or_else(|| RuneError::InvalidData("Final token transaction is none".to_string()))?;

        let final_token_transaction_hash = self
            .proto_hasher
            .hash_proto(
                final_token_transaction_proto
                    .to_dynamic()
                    .map_err(|e| RuneError::HashError(format!("Failed to hash final token transaction: {:?}", e)))?,
            )
            .map_err(|e| RuneError::HashError(format!("Failed to hash final token transaction: {:?}", e)))?;

        let mut signatures = vec![];

        for operator_public_key in self.spark_client.get_operator_public_keys() {
            let operator_specific_signable_payload =
                hash_operator_specific_signable_payload(final_token_transaction_hash, operator_public_key).map_err(
                    |err| RuneError::HashError(format!("Failed to hash operator specific signable payload: {:?}", err)),
                )?;

            let message = Message::from_digest(operator_specific_signable_payload.to_byte_array());
            let signature = secp.sign_schnorr_no_aux_rand(&message, &self.keypair);

            let input_ttxo_signatures_per_operator = InputTtxoSignaturesPerOperator {
                ttxo_signatures: vec![SignatureWithIndex {
                    signature: signature.serialize().to_vec(),
                    input_index: 0,
                }],
                operator_identity_public_key: operator_public_key.serialize().to_vec(),
            };

            signatures.push(input_ttxo_signatures_per_operator);
        }

        let commit_transaction_request = CommitTransactionRequest {
            final_token_transaction: Some(final_token_transaction_proto),
            final_token_transaction_hash: final_token_transaction_hash.to_byte_array().to_vec(),
            input_ttxo_signatures_per_operator: signatures,
            owner_identity_public_key: self.keypair.public_key().serialize().to_vec(),
        };

        let _ = self
            .spark_client
            .commit_spark_transaction(commit_transaction_request, self.keypair)
            .await?;

        Ok(())
    }

    pub async fn create_user_paying_transfer_input(
        &mut self,
        transfer_tx: Transaction,
    ) -> Result<UserPayingTransferInput, RuneError> {
        let txid = transfer_tx.compute_txid();
        let previous_output = OutPoint { txid, vout: 1 };
        let txin = TxIn {
            previous_output,
            script_sig: ScriptBuf::new(),
            sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
            witness: Witness::new(),
        };

        let transaction = Transaction {
            version: Version::TWO,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vec![txin],
            output: vec![],
        };

        let mut sighash_cache = SighashCache::new(&transaction);
        let txout = TxOut {
            value: Amount::from_sat(PAYING_INPUT_SATS_AMOUNT),
            script_pubkey: transfer_tx.output[1].script_pubkey.clone(),
        };
        let message_hash = sighash_cache
            .taproot_key_spend_signature_hash(0, &Prevouts::One(0, txout), TapSighashType::NonePlusAnyoneCanPay)
            .map_err(|e| RuneError::HashError(format!("Failed to create message hash: {}", e)))?;

        let message = Message::from_digest(message_hash.to_byte_array());
        let secp = Secp256k1::new();
        let tweaked = self.keypair.tap_tweak(&secp, None);
        let signature = secp.sign_schnorr_no_aux_rand(&message, &tweaked.to_keypair());

        let paying_input = UserPayingTransferInput {
            txid: txid.to_string(),
            vout: 1,
            btc_exit_address: self.p2tr_address.clone().to_string(),
            sats_amount: PAYING_INPUT_SATS_AMOUNT,
            none_anyone_can_pay_signature: signature,
        };

        Ok(paying_input)
    }
}

fn create_partial_token_leaf_output(
    receiver_identity_public_key: PublicKey,
    token_identifier: TokenIdentifier,
    token_amount: u128,
) -> TokenLeafOutput {
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
        withdrawal_bond_sats: None,
        withdrawal_locktime: None,
    }
}

fn hash_operator_specific_signable_payload(
    token_tx_hash: Sha256Hash,
    operator_public_key: PublicKey, // this must always be 33 bytes
) -> Result<Sha256Hash, Box<dyn std::error::Error>> {
    let mut engine = Sha256Hash::engine();
    engine.input(Sha256Hash::hash(token_tx_hash.as_byte_array().as_slice()).as_byte_array());
    engine.input(Sha256Hash::hash(operator_public_key.serialize().as_slice()).as_byte_array());
    let final_hash = Sha256Hash::from_engine(engine);

    Ok(final_hash)
}
