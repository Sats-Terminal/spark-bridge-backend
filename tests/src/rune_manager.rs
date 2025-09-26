use crate::bitcoin_client::BitcoinClient;
use bitcoin::Address;
use bitcoin::key::Keypair;
use rand_core::{OsRng, RngCore};
use crate::utils::create_credentials;
use crate::error::TestError;
use ordinals::RuneId;
use crate::rune_etching::{EtchRuneParams, etch_rune};
use bitcoin::{TxIn, OutPoint, Sequence, Witness, TxOut, Amount, ScriptBuf, Transaction, Txid};
use bitcoin::transaction::Version;
use tokio::time::sleep;
use std::time::Duration;
use titan_client::SpentStatus;
use ordinals::Runestone;
use std::str::FromStr;
use crate::utils::sign_transaction;

const DEFAULT_FEE_AMOUNT: u64 = 5_000;
const DEFAULT_DUST_AMOUNT: u64 = 546;

pub struct RuneManager {
    bitcoin_client: BitcoinClient,
    p2tr_address: Address,
    keypair: Keypair,
    rune_id: RuneId,
}

impl RuneManager {
    pub async fn new(mut bitcoin_client: BitcoinClient) -> Result<Self, TestError> {
        let (p2tr_address, keypair) = create_credentials();

        bitcoin_client.faucet(p2tr_address.clone(), 1_000_000)?;
        sleep(Duration::from_secs(1)).await;

        let rune_id = etch_rune(EtchRuneParams {
            rune_name: random_rune_name(),
            cap: 1_000,
            amount: 1_000_000,
            key_pair: keypair,
            faucet_address: p2tr_address.clone(),
        }, bitcoin_client.clone()).await?;

        let mut rune_manager = Self { bitcoin_client, p2tr_address, keypair, rune_id };
        let _ = rune_manager.unite_unspent_utxos().await?;
        rune_manager.bitcoin_client.generate_blocks(6, None)?;
        sleep(Duration::from_secs(1)).await;

        Ok(rune_manager)
    }

    pub async fn get_rune_id(&self) -> RuneId {
        self.rune_id.clone()
    }

    async fn unite_unspent_utxos(&mut self) -> Result<Txid, TestError> {
        tracing::info!("Uniting unspent utxos");

        let address_data = self.bitcoin_client.get_address_data(self.p2tr_address.clone()).await?;
        
        let mut total_amount = 0;
        let mut prev_input_amounts = vec![];
        let mut txins = vec![];

        for utxo in address_data.outputs.iter() {
            if !utxo.status.confirmed {
                return Err(TestError::UniteUnspentUtxosError("Unspent utxo is not confirmed".to_string()));
            }
            if let SpentStatus::Unspent = utxo.spent {
                total_amount += utxo.value;
                prev_input_amounts.push(utxo.value);

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

        let txout = TxOut {
            value: Amount::from_sat(total_amount - DEFAULT_FEE_AMOUNT),
            script_pubkey: self.p2tr_address.script_pubkey(),
        };
        
        let mut transaction = Transaction {
            version: Version::TWO,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: txins,
            output: vec![txout],
        };

        sign_transaction(&mut transaction, prev_input_amounts, self.p2tr_address.clone(), self.keypair)?;

        let txid = transaction.compute_txid();
        self.bitcoin_client.broadcast_transaction(transaction)?;
        self.bitcoin_client.generate_blocks(6, None)?;
        sleep(Duration::from_secs(1)).await;

        Ok(txid)
    }

    async fn get_funded_outpoint_data(&mut self) -> Result<(OutPoint, u64), TestError> {
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

        Err(TestError::GetFundedOutpointError("Failed to get funded outpoint".to_string()))
    }

    pub async fn mint_rune(&mut self, address: Address) -> Result<Txid, TestError> {
        tracing::info!("Minting rune");

        let runestone = Runestone {
            edicts: vec![],
            etching: None,
            mint: Some(self.rune_id),
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

        sign_transaction(&mut transaction, vec![value], address.clone(), self.keypair)?;

        let txid = transaction.compute_txid();
        self.bitcoin_client.broadcast_transaction(transaction)?;
        self.bitcoin_client.generate_blocks(6, None)?;
        sleep(Duration::from_secs(1)).await;

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
