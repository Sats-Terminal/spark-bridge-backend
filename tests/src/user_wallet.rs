use crate::utils::create_credentials;
use bitcoin::Address;
use bitcoin::key::Keypair;
use crate::bitcoin_client::BitcoinClient;
use crate::error::RuneError;
use tokio::time::sleep;
use std::time::Duration;
use ordinals::RuneId;
use titan_client::SpentStatus;
use bitcoin::{TxIn, OutPoint, ScriptBuf, Sequence, Witness, TxOut, Amount};
use ordinals::{Runestone, Edict};
use bitcoin::Transaction;
use bitcoin::transaction::Version;
use crate::utils::sign_transaction;
use bitcoin::Txid;
use tracing;
use crate::constants::{DEFAULT_FEE_AMOUNT, DEFAULT_DUST_AMOUNT, DEFAULT_FAUCET_AMOUNT, BLOCKS_TO_GENERATE};

pub struct UserWallet {
    p2tr_address: Address,
    keypair: Keypair,
    bitcoin_client: BitcoinClient,
    rune_id: RuneId,
}

impl UserWallet {
    pub async fn new(mut bitcoin_client: BitcoinClient, rune_id: RuneId) -> Result<Self, RuneError> {
        tracing::info!("Creating user wallet");
        let (p2tr_address, keypair) = create_credentials();

        bitcoin_client.faucet(p2tr_address.clone(), DEFAULT_FAUCET_AMOUNT)?;
        sleep(Duration::from_secs(1)).await;

        Ok(Self { p2tr_address, keypair, bitcoin_client, rune_id })
    }

    pub fn get_address(&self) -> Address {
        self.p2tr_address.clone()
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
                            return Ok((OutPoint { txid: output.txid, vout: output.vout }, output.value));
                        } 
                        tracing::warn!("There are funded runes, but the amount is less than 10_000");
                    }
                }
            }
        }
        Err(RuneError::GetFundedOutpointError("Failed to get funded outpoint".to_string()))
    }

    pub async fn transfer_runes(&mut self, amount: u64, transfer_address: Address) -> Result<Txid, RuneError> {
        tracing::info!("Transferring runes");
        let balance = self.get_rune_balance().await?;
        if balance < amount {
            return Err(RuneError::TransferRunesError("Insufficient balance".to_string()));
        }

        let (outpoint, value) = self.get_funded_outpoint_data().await?;

        let runestone = Runestone {
            edicts: vec![
                Edict {
                    id: self.rune_id,
                    amount: amount as u128,
                    output: 1,
                },
                Edict {
                    id: self.rune_id,
                    amount: (balance - amount) as u128,
                    output: 2,
                },
            ],
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

        let txouts = vec![
            TxOut {
                value: Amount::from_sat(0),
                script_pubkey: op_return_script,
            },
            TxOut {
                value: Amount::from_sat(DEFAULT_DUST_AMOUNT),
                script_pubkey: transfer_address.script_pubkey(),
            },
            TxOut {
                value: Amount::from_sat(value - DEFAULT_FEE_AMOUNT - DEFAULT_DUST_AMOUNT),
                script_pubkey: self.p2tr_address.script_pubkey(),
            },
        ];

        let mut transaction = Transaction {
            version: Version::TWO,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vec![txin],
            output: txouts,
        };
        
        sign_transaction(&mut transaction, vec![value], self.p2tr_address.clone(), self.keypair)?;

        let txid = transaction.compute_txid();

        self.bitcoin_client.broadcast_transaction(transaction)?;
        self.bitcoin_client.generate_blocks(BLOCKS_TO_GENERATE, None)?;
        sleep(Duration::from_secs(1)).await;

        tracing::info!("Runes transferred");

        Ok(txid)
    }

    pub async fn unite_unspent_utxos(&mut self) -> Result<Txid, RuneError> {
        let address_data = self.bitcoin_client.get_address_data(self.p2tr_address.clone()).await?;

        let mut total_btc = 0;
        let mut total_runes = 0;
        let mut txins = vec![];
        let mut prev_input_amounts = vec![];

        for output in address_data.outputs.iter() {
            if let SpentStatus::Unspent = output.spent && output.value > 0 {
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

        sign_transaction(&mut transaction, prev_input_amounts, self.p2tr_address.clone(), self.keypair)?;

        let txid = transaction.compute_txid();

        self.bitcoin_client.broadcast_transaction(transaction)?;
        self.bitcoin_client.generate_blocks(BLOCKS_TO_GENERATE, None)?;
        sleep(Duration::from_secs(1)).await;

        Ok(txid)
    }
}
