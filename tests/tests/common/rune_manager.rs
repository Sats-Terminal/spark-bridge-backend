use crate::common::bitcoin_client::BitcoinClient;
use crate::common::constants::{
    DEFAULT_DUST_AMOUNT, DEFAULT_FAUCET_AMOUNT, DEFAULT_FEE_AMOUNT, DEFAULT_RUNE_AMOUNT, DEFAULT_RUNE_CAP,
};
use crate::common::error::RuneError;
use crate::common::rune_etching::{EtchRuneParams, etch_rune};
use crate::common::utils::{create_credentials, sign_transaction};
use bitcoin::Network;
use bitcoin::{
    Address, Amount, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Txid, Witness, key::Keypair,
    transaction::Version,
};
use btc_indexer_client::client_api::AddrUtxoData;
use ordinals::{RuneId, Runestone};
use rand_core::{OsRng, RngCore};
use serde::Serialize;
use std::str::FromStr;

pub struct RuneManager {
    p2tr_address: Address,
    keypair: Keypair,
    rune_id: RuneId,
}

impl RuneManager {
    pub async fn new(
        p2tr_address: Address,
        keypair: Keypair,
        rune_id: RuneId,
        address_utxos_data: Vec<AddrUtxoData>,
    ) -> Result<(Self, Transaction), RuneError> {
        let mut rune_manager = Self {
            p2tr_address,
            keypair,
            rune_id,
        };
        let tx = rune_manager.unite_unspent_utxos(address_utxos_data).await?;

        Ok((rune_manager, tx))
    }

    pub fn get_rune_id(&self) -> RuneId {
        self.rune_id
    }

    pub fn get_p2tr_address(&self) -> Address {
        self.p2tr_address.clone()
    }

    async fn unite_unspent_utxos(&mut self, address_utxos_data: Vec<AddrUtxoData>) -> Result<Transaction, RuneError> {
        tracing::info!("Uniting unspent utxos");

        let mut total_amount = 0;
        let mut prev_input_amounts = vec![];
        let mut txins = vec![];

        for utxo_data in address_utxos_data.iter() {
            if !utxo_data.confirmed {
                return Err(RuneError::UniteUnspentUtxosError(
                    "Unspent utxo is not confirmed".to_string(),
                ));
            }
            if !utxo_data.spent {
                total_amount += utxo_data.value;
                prev_input_amounts.push(utxo_data.value);

                txins.push(TxIn {
                    previous_output: OutPoint {
                        txid: Txid::from_str(&utxo_data.txid).unwrap(),
                        vout: utxo_data.vout,
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

        sign_transaction(
            &mut transaction,
            prev_input_amounts,
            self.p2tr_address.clone(),
            self.keypair,
        )?;

        Ok(transaction)
    }

    async fn get_funded_outpoint_data(
        &self,
        address_utxos_data: Vec<AddrUtxoData>,
    ) -> Result<(OutPoint, u64), RuneError> {
        for utxo_data in address_utxos_data.iter() {
            if utxo_data.value >= 100_000 && !utxo_data.spent {
                return Ok((
                    OutPoint {
                        txid: Txid::from_str(&utxo_data.txid).unwrap(),
                        vout: utxo_data.vout,
                    },
                    utxo_data.value,
                ));
            }
        }

        Err(RuneError::GetFundedOutpointError(
            "Failed to get funded outpoint".to_string(),
        ))
    }

    pub async fn build_mint_tx(
        &self,
        address: Address,
        address_utxos_data: Vec<AddrUtxoData>,
    ) -> Result<Transaction, RuneError> {
        tracing::info!("Minting rune");

        let runestone = Runestone {
            edicts: vec![],
            etching: None,
            mint: Some(self.rune_id),
            pointer: Some(1),
        };
        let op_return_script = runestone.encipher();

        let (outpoint, value) = self.get_funded_outpoint_data(address_utxos_data).await?;

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
        Ok(transaction)
    }
}

pub fn random_rune_name() -> String {
    let letters = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let mut result = String::new();
    let mut rng = OsRng;
    for _ in 0..15 {
        let random_num = rng.next_u32() as usize % letters.len();
        let new_char = letters
            .chars()
            .nth(random_num)
            .expect("should be able to generate a random rune name");
        result.push(new_char);
    }
    result
}

pub async fn setup_rune_manager(
    bitcoin_client: &mut impl BitcoinClient,
    network: Network,
    private_key: Option<&str>,
    rune_id: Option<RuneId>,
) -> (RuneManager, Transaction) {
    let (p2tr_address, keypair) = create_credentials(network, private_key);
    bitcoin_client
        .faucet(p2tr_address.clone(), DEFAULT_FAUCET_AMOUNT)
        .await
        .unwrap();

    let rune_id = if rune_id.is_none() {
        etch_rune(
            EtchRuneParams {
                network: network,
                rune_name: random_rune_name(),
                cap: DEFAULT_RUNE_CAP,
                amount: DEFAULT_RUNE_AMOUNT,
                key_pair: keypair,
                faucet_address: p2tr_address.clone(),
            },
            bitcoin_client,
        )
        .await
        .unwrap()
    } else {
        rune_id.unwrap()
    };

    let utxos_data = bitcoin_client.get_address_data(p2tr_address.clone()).await.unwrap();
    let (rune_manager, transaction) = RuneManager::new(p2tr_address.clone(), keypair, rune_id, utxos_data)
        .await
        .unwrap();

    (rune_manager, transaction)
}
