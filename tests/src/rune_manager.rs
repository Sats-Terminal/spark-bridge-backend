use crate::bitcoin_client::BitcoinClient;
use bitcoin::Address;
use bitcoin::key::Keypair;
use rand_core::{OsRng, RngCore};
use crate::utils::create_credentials;
use crate::error::{RuneError};
use ordinals::{Edict, RuneId, Runestone};
use crate::rune_etching::{EtchRuneParams, etch_rune};
use bitcoin::{TxIn, OutPoint, Sequence, Witness, TxOut, Amount, ScriptBuf, Transaction, Txid};
use bitcoin::transaction::Version;
use tokio::time::sleep;
use std::time::Duration;
use titan_client::SpentStatus;
use std::str::FromStr;
use crate::utils::sign_transaction;
use std::collections::HashMap;

const DEFAULT_FEE_AMOUNT: u64 = 5_000;
const DEFAULT_DUST_AMOUNT: u64 = 546;

pub struct RuneManager {
    bitcoin_client: BitcoinClient,
    p2tr_address: Address,
    keypair: Keypair,
    managed_runes: HashMap<RuneId, RuneInfo>,
}

#[derive(Debug, Clone)]
pub struct RuneInfo {
    pub rune_id: RuneId,
    pub name: String,
    pub cap: u128,
    pub amount_per_mint: u128,
}

impl RuneManager {
    pub async fn new(mut bitcoin_client: BitcoinClient) -> Result<Self, RuneError> {
        let (p2tr_address, keypair) = create_credentials();

        bitcoin_client.faucet(p2tr_address.clone(), 1_000_000)?;
        sleep(Duration::from_secs(1)).await;

        Ok(Self {
            bitcoin_client,
            p2tr_address,
            keypair,
            managed_runes: HashMap::new()
        })
    }

    pub async fn new_with_rune(mut bitcoin_client: BitcoinClient) -> Result<Self, RuneError> {
        let (p2tr_address, keypair) = create_credentials();

        bitcoin_client.faucet(p2tr_address.clone(), 1_000_000)?;
        sleep(Duration::from_secs(1)).await;

        let rune_name = random_rune_name();
        let rune_id = etch_rune(EtchRuneParams {
            rune_name: rune_name.clone(),
            cap: 1_000,
            amount: 1_000_000,
            key_pair: keypair,
            faucet_address: p2tr_address.clone(),
        }, bitcoin_client.clone()).await?;

        let mut managed_runes = HashMap::new();
        managed_runes.insert(rune_id, RuneInfo {
            rune_id,
            name: rune_name,
            cap: 1_000,
            amount_per_mint: 1_000_000,
        });

        let mut rune_manager = Self { bitcoin_client, p2tr_address, keypair, managed_runes };
        let _ = rune_manager.unite_unspent_utxos().await?;
        rune_manager.bitcoin_client.generate_blocks(6, None)?;
        sleep(Duration::from_secs(1)).await;

        Ok(rune_manager)
    }



    pub async fn etch_new_rune(
        &mut self,
        rune_name: Option<String>,
        cap: u128,
        amount: u128,
    ) -> Result<RuneId, RuneError> {
        tracing::info!("Etching new rune");

        let name = rune_name.unwrap_or_else(random_rune_name);

        let rune_id = etch_rune(EtchRuneParams {
            rune_name: name.clone(),
            cap: cap as u64,
            amount: amount as u64,
            key_pair: self.keypair,
            faucet_address: self.p2tr_address.clone(),
        }, self.bitcoin_client.clone()).await?;

        self.managed_runes.insert(rune_id, RuneInfo {
            rune_id,
            name,
            cap,
            amount_per_mint: amount,
        });

        let _ = self.unite_unspent_utxos().await?;
        self.bitcoin_client.generate_blocks(6, None)?;
        sleep(Duration::from_secs(10)).await;


        Ok(rune_id)
    }

    pub fn get_managed_runes(&self) -> HashMap<RuneId, RuneInfo> {
        self.managed_runes.clone()
    }

    pub fn get_rune_info(&self, rune_id: &RuneId) -> Option<&RuneInfo> {
        self.managed_runes.get(rune_id)
    }

    pub async fn get_rune_id(&self) -> Option<RuneId> {
        self.managed_runes.keys().next().copied()
    }

    pub fn get_all_rune_ids(&self) -> Vec<RuneId> {
        self.managed_runes.keys().copied().collect()
    }

    async fn unite_unspent_utxos(&mut self) -> Result<Txid, RuneError> {
        tracing::info!("Uniting unspent utxos");

        let address_data = self.bitcoin_client.get_address_data(self.p2tr_address.clone()).await?;

        let mut total_amount = 0;
        let mut prev_input_amounts = vec![];
        let mut txins = vec![];
        let mut rune_totals: HashMap<RuneId, u128> = HashMap::new();

        for utxo in address_data.outputs.iter() {
            if !utxo.status.confirmed {
                return Err(RuneError::UniteUnspentUtxosError("Unspent utxo is not confirmed".to_string()));
            }
            if let SpentStatus::Unspent = utxo.spent {
                total_amount += utxo.value;
                prev_input_amounts.push(utxo.value);

                for rune in utxo.runes.iter() {
                    let rune_id = RuneId::from_str(&rune.rune_id.to_string())
                        .map_err(|e| RuneError::UniteUnspentUtxosError(format!("Failed to parse RuneId: {}", e)))?;
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

        let mut edicts = vec![];
        for (rune_id, total) in rune_totals.iter() {
            if *total > 0 {
                edicts.push(Edict {
                    id: *rune_id,
                    amount: *total,
                    output: 1,
                });
            }
        }

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
        sleep(Duration::from_secs(5)).await;

        Ok(txid)
    }

    async fn get_funded_outpoint_data(&mut self) -> Result<(OutPoint, u64), RuneError> {
        let address_data = self.bitcoin_client.get_address_data(self.p2tr_address.clone()).await?;

        for output in address_data.outputs.iter() {
            if output.value >= 100_000 {
                if let SpentStatus::Unspent = output.spent {
                    return Ok((OutPoint {
                        txid: Txid::from_str(&output.txid.to_string()).unwrap(),
                        vout: output.vout,
                    }, output.value));
                }
            }
        }

        Err(RuneError::GetFundedOutpointError("Failed to get funded outpoint".to_string()))
    }

    pub async fn mint_rune(&mut self, rune_id: RuneId, address: Address) -> Result<Txid, RuneError> {
        tracing::info!("Minting rune {:?}", rune_id);

        if !self.managed_runes.contains_key(&rune_id) {
            return Err(RuneError::MintRuneError(
                format!("Rune {:?} is not managed by this RuneManager", rune_id)
            ));
        }

        let runestone = Runestone {
            edicts: vec![],
            etching: None,
            mint: Some(rune_id),
            pointer: Some(1),
        };
        let op_return_script = runestone.encipher();

        let (outpoint, value) = self.get_funded_outpoint_data().await?;

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
                script_pubkey: address.script_pubkey(),
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
        tracing::info!("MINT TRANSACTION TXID: {}", txid);
        self.bitcoin_client.generate_blocks(6, None)?;
        sleep(Duration::from_secs(5)).await;

        Ok(txid)
    }

}

pub fn random_rune_name() -> String {
    let letters = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let mut result = String::new();
    let mut rng = OsRng;
    for _ in 0..15 {
        let random_num = rng.next_u32() as usize % letters.len();
        let new_char = letters.chars().nth(random_num).expect("should be able to generate a random rune name");
        result.push(new_char);
    }
    result
}