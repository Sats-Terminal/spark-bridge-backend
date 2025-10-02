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
use std::collections::HashMap;
use std::str::FromStr;

const DEFAULT_FEE_AMOUNT: u64 = 5_000;
const DEFAULT_DUST_AMOUNT: u64 = 546;

pub struct UserWallet {
    p2tr_address: Address,
    keypair: Keypair,
    bitcoin_client: BitcoinClient,
    rune_ids: Vec<RuneId>,
}

impl UserWallet {
    pub async fn new(mut bitcoin_client: BitcoinClient, rune_ids: Vec<RuneId>) -> Result<Self, RuneError> {
        tracing::info!("Creating user wallet with {} runes", rune_ids.len());
        let (p2tr_address, keypair) = create_credentials();

        bitcoin_client.faucet(p2tr_address.clone(), 1_000_000)?;
        sleep(Duration::from_secs(1)).await;

        Ok(Self { p2tr_address, keypair, bitcoin_client, rune_ids })
    }

    pub fn get_address(&self) -> Address {
        self.p2tr_address.clone()
    }

    pub fn get_rune_ids(&self) -> Vec<RuneId> {
        self.rune_ids.clone()
    }

    pub async fn get_rune_balance(&self, rune_id: &RuneId) -> Result<u64, RuneError> {
        let address_data = self.bitcoin_client.get_address_data(self.p2tr_address.clone()).await?;
        let mut total_balance = 0;
        for output in address_data.outputs.iter() {
            if let SpentStatus::Unspent = output.spent {
                for runes in output.runes.iter() {
                    if runes.rune_id.to_string() == rune_id.to_string() {
                        total_balance += runes.amount;
                    }
                }
            }
        }
        Ok(total_balance as u64)
    }

    pub async fn get_all_balances(&self) -> Result<HashMap<RuneId, u64>, RuneError> {
        let address_data = self.bitcoin_client.get_address_data(self.p2tr_address.clone()).await?;
        let mut balances: HashMap<RuneId, u64> = HashMap::new();

        for output in address_data.outputs.iter() {
            if let SpentStatus::Unspent = output.spent {
                for runes in output.runes.iter() {
                    let rune_id = RuneId::from_str(&runes.rune_id.to_string())
                        .map_err(|e| RuneError::GetRuneBalanceError(format!("Failed to parse RuneId: {}", e)))?;
                    *balances.entry(rune_id).or_insert(0) += runes.amount as u64;
                }
            }
        }

        Ok(balances)
    }

    pub async fn get_funded_outpoint_data(&self, rune_id: &RuneId) -> Result<(OutPoint, u64), RuneError> {
        let address_data = self.bitcoin_client.get_address_data(self.p2tr_address.clone()).await?;
        for output in address_data.outputs.iter() {
            if let SpentStatus::Unspent = output.spent {
                for runes in output.runes.iter() {
                    if runes.rune_id.to_string() == rune_id.to_string() {
                        if output.value >= 100_000 && runes.amount >= 10_000 {
                            return Ok((OutPoint { txid: output.txid, vout: output.vout }, output.value));
                        }
                        tracing::warn!("There are funded runes, but the amount is less than 10_000");
                    }
                }
            }
        }
        Err(RuneError::GetFundedOutpointError(format!("Failed to get funded outpoint for rune {:?}", rune_id)))
    }

    pub async fn transfer_runes(&mut self, rune_id: RuneId, amount: u64, transfer_address: Address) -> Result<Txid, RuneError> {
        tracing::info!("Transferring {} of rune {:?}", amount, rune_id);

        let balance = self.get_rune_balance(&rune_id).await?;
        if balance < amount {
            return Err(RuneError::TransferRunesError(format!("Insufficient balance for rune {:?}. Have: {}, Need: {}", rune_id, balance, amount)));
        }

        let (outpoint, value) = self.get_funded_outpoint_data(&rune_id).await?;

        let runestone = Runestone {
            edicts: vec![
                Edict {
                    id: rune_id,
                    amount: amount as u128,
                    output: 1,
                },
                Edict {
                    id: rune_id,
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
        self.bitcoin_client.generate_blocks(6, None)?;
        sleep(Duration::from_secs(1)).await;

        tracing::info!("Runes transferred successfully");

        Ok(txid)
    }

    pub async fn unite_unspent_utxos(&mut self) -> Result<Txid, RuneError> {
        tracing::info!("Uniting unspent UTXOs");
        tracing::info!("Wallet address: {}", self.p2tr_address);

        tokio::time::sleep(Duration::from_secs(3)).await;

        let address_data = self.bitcoin_client.get_address_data(self.p2tr_address.clone()).await?;

        let mut total_amount = 0;
        let mut prev_input_amounts = vec![];
        let mut txins = vec![];
        let mut rune_totals: HashMap<RuneId, u128> = HashMap::new();

        tracing::debug!("Found {} outputs", address_data.outputs.len());

        for utxo in address_data.outputs.iter() {
            if !utxo.status.confirmed {
                tracing::warn!("Skipping unconfirmed UTXO: {}:{}", utxo.txid, utxo.vout);
                continue;
            }
            if let SpentStatus::Unspent = utxo.spent {
                tracing::debug!("UTXO {}:{} has {} runes", utxo.txid, utxo.vout, utxo.runes.len());

                total_amount += utxo.value;
                prev_input_amounts.push(utxo.value);

                for rune in utxo.runes.iter() {
                    let rune_id = RuneId::from_str(&rune.rune_id.to_string())
                        .map_err(|e| RuneError::UniteUnspentUtxosError(format!("Failed to parse RuneId: {}", e)))?;
                    tracing::debug!("Found rune {:?} with amount {}", rune_id, rune.amount);
                    *rune_totals.entry(rune_id).or_insert(0) += rune.amount;
                }

                txins.push(TxIn {
                    previous_output: OutPoint {
                        txid: utxo.txid,
                        vout: utxo.vout,
                    },
                    script_sig: ScriptBuf::new(),
                    sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
                    witness: Witness::new(),
                });
            }
        }

        tracing::info!("Total unique runes found: {}", rune_totals.len());

        if txins.is_empty() {
            return Err(RuneError::UniteUnspentUtxosError("No unspent UTXOs found".to_string()));
        }

        let mut edicts = vec![];
        for (rune_id, total) in rune_totals.iter() {
            if *total > 0 {
                tracing::info!("Creating edict for rune {:?}: amount {}", rune_id, total);
                edicts.push(Edict {
                    id: *rune_id,
                    amount: *total,
                    output: 1,
                });
            }
        }

        tracing::info!("Total edicts created: {}", edicts.len());

        let mut outputs = vec![];

        if !edicts.is_empty() {
            let runestone = Runestone {
                edicts,
                etching: None,
                mint: None,
                pointer: None,
            };
            outputs.push(TxOut {
                value: Amount::from_sat(0),
                script_pubkey: runestone.encipher(),
            });
        }

        outputs.push(TxOut {
            value: Amount::from_sat(total_amount - DEFAULT_FEE_AMOUNT),
            script_pubkey: self.p2tr_address.script_pubkey(),
        });

        let mut transaction = Transaction {
            version: Version::TWO,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: txins,
            output: outputs,
        };

        sign_transaction(&mut transaction, prev_input_amounts, self.p2tr_address.clone(), self.keypair)?;

        let txid = transaction.compute_txid();
        self.bitcoin_client.broadcast_transaction(transaction)?;
        self.bitcoin_client.generate_blocks(6, None)?;

        tokio::time::sleep(Duration::from_secs(5)).await;

        tracing::info!("UTXOs united successfully");
        Ok(txid)
    }
}