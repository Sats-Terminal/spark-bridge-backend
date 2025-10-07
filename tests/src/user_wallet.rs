use crate::bitcoin_client::BitcoinClient;
use crate::constants::{
    BLOCKS_TO_GENERATE, DEFAULT_DUST_AMOUNT, DEFAULT_FAUCET_AMOUNT, DEFAULT_FEE_AMOUNT, PAYING_INPUT_SATS_AMOUNT,
};
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
use chrono::Utc;
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
use std::time::Duration;
use titan_client::SpentStatus;
use token_identifier::TokenIdentifier;
use tokio::time::sleep;
use tracing;
use uuid::Uuid;

pub enum TransferType {
    RuneTransfer { rune_amount: u64 },
    BtcTransfer { sats_amount: u64 },
}

pub struct UserWallet {
    p2tr_address: Address,
    keypair: Keypair,
    bitcoin_client: BitcoinClient,
    spark_client: SparkClient,
    rune_id: RuneId,
    proto_hasher: ProtoHasher,
    user_id: Uuid,
}

impl UserWallet {
    pub async fn new(
        mut bitcoin_client: BitcoinClient,
        spark_client: SparkClient,
        rune_id: RuneId,
    ) -> Result<Self, RuneError> {
        tracing::info!("Creating user wallet");
        let (p2tr_address, keypair) = create_credentials();

        bitcoin_client.faucet(p2tr_address.clone(), DEFAULT_FAUCET_AMOUNT)?;
        sleep(Duration::from_secs(1)).await;

        let proto_hasher = ProtoHasher::new();

        Ok(Self {
            p2tr_address,
            keypair,
            bitcoin_client,
            spark_client,
            rune_id,
            proto_hasher,
            user_id: Uuid::new_v4(),
        })
    }

    pub fn get_address(&self) -> Address {
        self.p2tr_address.clone()
    }

    pub fn get_user_id(&self) -> Uuid {
        self.user_id.clone()
    }

    pub fn get_spark_address(&self) -> Result<String, RuneError> {
        let identity_public_key = self.keypair.public_key();
        let spark_address = encode_spark_address(SparkAddressData {
            identity_public_key: identity_public_key.to_string(),
            network: spark_address::Network::Regtest,
            invoice: None,
            signature: None,
        })?;
        Ok(spark_address)
    }

    pub async fn get_rune_balance(&self) -> Result<u64, RuneError> {
        let address_data = self.bitcoin_client.get_address_data(self.p2tr_address.clone()).await?;
        let mut total_balance = 0;
        for output in address_data.outputs.iter() {
            if let SpentStatus::Unspent = output.spent {
                for runes in output.runes.iter() {
                    if runes.rune_id.to_string() == self.rune_id.to_string() {
                        total_balance += runes.amount;
                    }
                }
            }
        }
        Ok(total_balance as u64)
    }

    pub async fn get_funded_outpoint_data(&self) -> Result<(OutPoint, u64), RuneError> {
        let address_data = self.bitcoin_client.get_address_data(self.p2tr_address.clone()).await?;
        for output in address_data.outputs.iter() {
            if let SpentStatus::Unspent = output.spent {
                for runes in output.runes.iter() {
                    if runes.rune_id.to_string() == self.rune_id.to_string() {
                        if output.value >= 100_000 && runes.amount >= 10_000 {
                            return Ok((
                                OutPoint {
                                    txid: output.txid,
                                    vout: output.vout,
                                },
                                output.value,
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

    pub async fn transfer(
        &mut self,
        transfer_type: TransferType,
        transfer_address: Address,
    ) -> Result<Transaction, RuneError> {
        tracing::info!("Transferring runes");
        let rune_balance = self.get_rune_balance().await?;

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

        let (outpoint, value) = self.get_funded_outpoint_data().await?;

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

        let txid = transaction.compute_txid();

        self.bitcoin_client.broadcast_transaction(transaction.clone())?;
        self.bitcoin_client.generate_blocks(BLOCKS_TO_GENERATE, None)?;
        sleep(Duration::from_secs(1)).await;

        tracing::info!("Runes transferred");

        Ok(transaction)
    }

    pub async fn unite_unspent_utxos(&mut self) -> Result<Txid, RuneError> {
        let address_data = self.bitcoin_client.get_address_data(self.p2tr_address.clone()).await?;

        let mut total_btc = 0;
        let mut total_runes = 0;
        let mut txins = vec![];
        let mut prev_input_amounts = vec![];

        for output in address_data.outputs.iter() {
            if let SpentStatus::Unspent = output.spent
                && output.value > 0
            {
                total_btc += output.value;
                for runes in output.runes.iter() {
                    if runes.rune_id.to_string() == self.rune_id.to_string() {
                        total_runes += runes.amount;
                    }
                }
                txins.push(TxIn {
                    previous_output: OutPoint {
                        txid: output.txid,
                        vout: output.vout,
                    },
                    script_sig: ScriptBuf::new(),
                    sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
                    witness: Witness::new(),
                });
                prev_input_amounts.push(output.value);
            }
        }

        let runestone = Runestone {
            edicts: vec![Edict {
                id: self.rune_id,
                amount: total_runes,
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

        let txid = transaction.compute_txid();

        self.bitcoin_client.broadcast_transaction(transaction)?;
        self.bitcoin_client.generate_blocks(BLOCKS_TO_GENERATE, None)?;
        sleep(Duration::from_secs(1)).await;

        Ok(txid)
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
                parent_leaf_hash: Sha256Hash::from_bytes_ref(
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
                )
                .clone(),
                parent_leaf_index: token_output.prev_token_transaction_vout,
            });
        }

        let mut leaves_to_create = vec![create_partial_token_leaf_output(
            receiver_identity_public_key,
            token_identifier,
            transfer_amount as u128,
        )];

        tracing::debug!(
            "Token identifier: {:?}",
            token_identifier.encode_bech32m(bitcoin::Network::Regtest)
        );
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
            network: Some(2), // regtest, find spark_network_to_proto_network
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
            .start_spark_transaction(start_transaction_request, self.keypair.clone())
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
            .commit_spark_transaction(commit_transaction_request, self.keypair.clone())
            .await?;

        Ok(())
    }

    pub async fn create_user_paying_transfer_input(&mut self) -> Result<UserPayingTransferInput, RuneError> {
        let tx = self
            .transfer(
                TransferType::BtcTransfer {
                    sats_amount: PAYING_INPUT_SATS_AMOUNT,
                },
                self.p2tr_address.clone(),
            )
            .await?;
        let txid = tx.compute_txid();

        let previous_output = OutPoint { txid: txid, vout: 1 };
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
            script_pubkey: tx.output[1].script_pubkey.clone(),
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
