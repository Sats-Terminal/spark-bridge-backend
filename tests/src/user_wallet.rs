use crate::utils::create_credentials;
use bitcoin::Address;
use bitcoin::key::Keypair;
use crate::bitcoin_client::BitcoinClient;
use crate::error::TestError;
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
    pub async fn new(mut bitcoin_client: BitcoinClient, rune_ids: Vec<RuneId>) -> Result<Self, TestError> {
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

    pub async fn get_rune_balance(&self, rune_id: &RuneId) -> Result<u64, TestError> {
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

    pub async fn get_all_balances(&self) -> Result<HashMap<RuneId, u64>, TestError> {
        let address_data = self.bitcoin_client.get_address_data(self.p2tr_address.clone()).await?;
        let mut balances: HashMap<RuneId, u64> = HashMap::new();

        for output in address_data.outputs.iter() {
            if let SpentStatus::Unspent = output.spent {
                for runes in output.runes.iter() {
                    let rune_id = RuneId::from_str(&runes.rune_id.to_string())
                        .map_err(|e| TestError::GetRuneBalanceError(format!("Failed to parse RuneId: {}", e)))?;
                    *balances.entry(rune_id).or_insert(0) += runes.amount as u64;
                }
            }
        }

        Ok(balances)
    }

    pub async fn get_funded_outpoint_data(&self, rune_id: &RuneId) -> Result<(OutPoint, u64), TestError> {
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
        Err(TestError::GetFundedOutpointError(format!("Failed to get funded outpoint for rune {:?}", rune_id)))
    }

    pub async fn transfer_runes(&mut self, rune_id: RuneId, amount: u64, transfer_address: Address) -> Result<Txid, TestError> {
        tracing::info!("Transferring {} of rune {:?}", amount, rune_id);

        let balance = self.get_rune_balance(&rune_id).await?;
        if balance < amount {
            return Err(TestError::TransferRunesError(format!("Insufficient balance for rune {:?}. Have: {}, Need: {}", rune_id, balance, amount)));
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

    pub async fn unite_unspent_utxos(&mut self) -> Result<Txid, TestError> {
        tracing::info!("Uniting unspent UTXOs");
        tracing::info!("Wallet address: {}", self.p2tr_address);

        sleep(Duration::from_secs(10)).await;

        let address_data = {
            let mut retry_count = 0;
            let max_retries = 5;

            loop {
                let data = self.bitcoin_client.get_address_data(self.p2tr_address.clone()).await?;

                let mut found_runes = std::collections::HashSet::new();
                for output in data.outputs.iter() {
                    if let SpentStatus::Unspent = output.spent {
                        for rune in output.runes.iter() {
                            if let Ok(rune_id) = RuneId::from_str(&rune.rune_id.to_string()) {
                                found_runes.insert(rune_id);
                            }
                        }
                    }
                }

                tracing::debug!("Found {} unique runes in unspent outputs", found_runes.len());

                if !found_runes.is_empty() || retry_count >= max_retries {
                    break data;
                }

                tracing::debug!("No runes found yet (attempt {}/{}), waiting...", retry_count + 1, max_retries);
                sleep(Duration::from_secs(2)).await;
                retry_count += 1;
            }
        };

        let mut total_btc = 0;
        let mut rune_totals: HashMap<RuneId, u128> = HashMap::new();
        let mut txins = vec![];
        let mut prev_input_amounts = vec![];

        for output in address_data.outputs.iter() {
            if let SpentStatus::Unspent = output.spent {
                if output.value > 0 {
                    total_btc += output.value;
                    tracing::debug!("UTXO {:?}:{} has {} runes", output.txid, output.vout, output.runes.len());
                    for runes in output.runes.iter() {
                        let rune_id = RuneId::from_str(&runes.rune_id.to_string())
                            .map_err(|e| TestError::UniteUnspentUtxosError(format!("Failed to parse RuneId: {}", e)))?;
                        tracing::debug!("Found rune {:?} with amount {}", rune_id, runes.amount);
                        *rune_totals.entry(rune_id).or_insert(0) += runes.amount;
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
        }

        if txins.is_empty() {
            return Err(TestError::UniteUnspentUtxosError("No unspent UTXOs to unite".to_string()));
        }

        tracing::info!("Total unique runes found: {}", rune_totals.len());

        let mut edicts = vec![];
        for (rune_id, total_amount) in rune_totals.iter() {
            if *total_amount > 0 {
                tracing::info!("Creating edict for rune {:?}: amount {}", rune_id, total_amount);
                edicts.push(Edict {
                    id: *rune_id,
                    amount: *total_amount,
                    output: 1,
                });
            }
        }

        tracing::info!("Total edicts created: {}", edicts.len());

        let runestone = Runestone {
            edicts,
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
        self.bitcoin_client.generate_blocks(6, None)?;
        sleep(Duration::from_secs(1)).await;

        tracing::info!("UTXOs united successfully");

        Ok(txid)
    }
}