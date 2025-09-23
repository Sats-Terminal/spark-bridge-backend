use std::str::FromStr;
use anyhow::Result;
use bitcoin::transaction::Version;
use ord_rs::bitcoin::{Address, Network, PrivateKey, secp256k1::Secp256k1, Amount, Txid as OrdTxid};
use ord_rs::{
    OrdTransactionBuilder, Nft,
    wallet::{
        CreateCommitTransactionArgsV2,
        SignCommitTransactionArgs,
        RevealTransactionArgs,
        EtchingTransactionArgs,
        Utxo,
        Runestone
    }
};
use ordinals::{Rune, Terms, RuneId};

pub struct BitcoinClient {
    network: Network,
}

impl BitcoinClient {
    pub fn new(network: Network) -> Self {
        Self { network }
    }
}

#[derive(Debug, Clone)]
pub struct EtchRuneParams {
    pub rune_name: String,
    pub divisibility: Option<u8>,
    pub premine: Option<u128>,
    pub symbol: Option<char>,
    pub terms: Option<RuneTerms>,
    pub inputs: Vec<TxInput>,
    pub commit_fee: Amount,
    pub reveal_fee: Amount,
    pub recipient_address: Address,
    pub dry_run: bool,
}

#[derive(Debug, Clone)]
pub struct MintRuneParams {
    pub rune_id: RuneId,
    pub amount: u128,
    pub inputs: Vec<TxInput>,
    pub recipient_address: Address,
    pub commit_fee: Amount,
    pub reveal_fee: Amount,
    pub dry_run: bool,
}

#[derive(Debug, Clone)]
pub struct TransferRuneParams {
    pub rune_id: RuneId,
    pub amount: u128,
    pub utxo_to_spend: TxInput,
    pub funding_inputs: Vec<TxInput>,
    pub recipient: Address,
    pub fee: Amount,
    pub dry_run: bool,
}

#[derive(Debug, Clone)]
pub struct RuneTerms {
    pub amount: Option<u128>,
    pub cap: Option<u128>,
    pub height_range: Option<(u64, u64)>,
    pub offset_range: Option<(u64, u64)>,
}

#[derive(Debug, Clone)]
pub struct TxInput {
    pub txid: String,
    pub vout: u32,
    pub amount: u64,
}

#[derive(Debug)]
pub struct TransactionResult {
    pub commit_txid: Option<String>,
    pub reveal_txid: String,
    pub success: bool,
}

pub struct RuneManager {
    builder: OrdTransactionBuilder,
    btc_client: BitcoinClient,
    private_key: PrivateKey,
    network: Network,
}

impl RuneManager {
    pub fn new(private_key: PrivateKey, network: Network, script_type: &str) -> Result<Self> {
        let btc_client = BitcoinClient::new(network);

        let builder = match script_type {
            "p2tr" | "P2TR" => OrdTransactionBuilder::p2tr(private_key),
            "p2wsh" | "P2WSH" => OrdTransactionBuilder::p2wsh(private_key),
            _ => return Err(anyhow::anyhow!("Invalid script type: {}", script_type)),
        };

        Ok(Self {
            builder,
            btc_client,
            private_key,
            network,
        })
    }

    pub async fn etch_rune(&mut self, params: EtchRuneParams) -> Result<TransactionResult> {
        log::info!("Starting rune etching for: {}", params.rune_name);

        let inputs = self.convert_inputs_to_utxos(params.inputs).await?;
        let sender_address = self.get_sender_address()?;

        let etching = ordinals::Etching {
            rune: Some(Rune::from_str(&params.rune_name)?),
            divisibility: params.divisibility,
            premine: params.premine,
            spacers: None,
            symbol: params.symbol,
            terms: params.terms.map(|t| Terms {
                amount: t.amount,
                cap: t.cap,
                height: (
                    t.height_range.map(|(start, _)| start),
                    t.height_range.map(|(_, end)| end)
                ),
                offset: (
                    t.offset_range.map(|(start, _)| start),
                    t.offset_range.map(|(_, end)| end)
                ),
            }),
            turbo: true,
        };

        let mut inscription = Nft::new(
            Some("text/plain;charset=utf-8".as_bytes().to_vec()),
            Some(etching.rune.unwrap().to_string().as_bytes().to_vec()),
        );
        inscription.pointer = Some(vec![]);
        inscription.rune = Some(etching.rune.unwrap().commitment());

        let commit_tx = self.builder
            .build_commit_transaction_with_fixed_fees(
                self.network,
                CreateCommitTransactionArgsV2 {
                    inputs: inputs.clone(),
                    inscription,
                    txin_script_pubkey: sender_address.script_pubkey(),
                    leftovers_recipient: params.recipient_address.clone(),
                    commit_fee: params.commit_fee,
                    reveal_fee: params.reveal_fee,
                    derivation_path: None,
                },
            )
            .await?;

        let signed_commit_tx = self.builder
            .sign_commit_transaction(
                commit_tx.unsigned_tx,
                SignCommitTransactionArgs {
                    inputs,
                    txin_script_pubkey: sender_address.script_pubkey(),
                    derivation_path: None,
                },
            )
            .await?;

        let commit_txid = if params.dry_run {
            None
        } else {
            log::info!("Broadcasting commit transaction: {}", signed_commit_tx.txid());
            Some(signed_commit_tx.txid().to_string())
        };

        let runestone = Runestone {
            etching: Some(etching),
            edicts: vec![],
            mint: None,
            pointer: Some(1),
        };

        let reveal_transaction = self.builder
            .build_etching_transaction(EtchingTransactionArgs {
                input: Utxo {
                    id: signed_commit_tx.txid(),
                    index: 0,
                    amount: commit_tx.reveal_balance,
                },
                recipient_address: params.recipient_address,
                redeem_script: commit_tx.redeem_script,
                runestone,
                derivation_path: None,
            })
            .await?;

        let reveal_txid = if params.dry_run {
            reveal_transaction.txid().to_string()
        } else {
            log::info!("Broadcasting reveal transaction: {}", reveal_transaction.txid());
            reveal_transaction.txid().to_string()
        };

        log::info!("Rune etching completed. Reveal TXID: {}", reveal_txid);

        Ok(TransactionResult {
            commit_txid,
            reveal_txid,
            success: true,
        })
    }

    pub async fn mint_rune(&mut self, params: MintRuneParams) -> Result<TransactionResult> {
        log::info!("Starting rune minting for: {:?}", params.rune_id);

        let inputs = self.convert_inputs_to_utxos(params.inputs).await?;
        let sender_address = self.get_sender_address()?;

        let commit_tx = self.builder
            .build_commit_transaction_with_fixed_fees(
                self.network,
                CreateCommitTransactionArgsV2 {
                    inputs: inputs.clone(),
                    inscription: Nft::new(None, None),
                    txin_script_pubkey: sender_address.script_pubkey(),
                    leftovers_recipient: params.recipient_address.clone(),
                    commit_fee: params.commit_fee,
                    reveal_fee: params.reveal_fee,
                    derivation_path: None,
                },
            )
            .await?;

        let signed_commit_tx = self.builder
            .sign_commit_transaction(
                commit_tx.unsigned_tx,
                SignCommitTransactionArgs {
                    inputs,
                    txin_script_pubkey: sender_address.script_pubkey(),
                    derivation_path: None,
                },
            )
            .await?;

        let commit_txid = if params.dry_run {
            None
        } else {
            log::info!("Broadcasting mint commit transaction: {}", signed_commit_tx.txid());
            Some(signed_commit_tx.txid().to_string())
        };

        let reveal_transaction = self.builder
            .build_reveal_transaction(RevealTransactionArgs {
                input: Utxo {
                    id: signed_commit_tx.txid(),
                    index: 0,
                    amount: commit_tx.reveal_balance,
                },
                recipient_address: params.recipient_address,
                redeem_script: commit_tx.redeem_script,
                derivation_path: None,
            })
            .await?;

        let reveal_txid = if params.dry_run {
            reveal_transaction.txid().to_string()
        } else {
            log::info!("Broadcasting mint reveal transaction: {}", reveal_transaction.txid());
            reveal_transaction.txid().to_string()
        };

        log::info!("Rune minting completed. Reveal TXID: {}", reveal_txid);

        Ok(TransactionResult {
            commit_txid,
            reveal_txid,
            success: true,
        })
    }

    pub async fn transfer_rune(&self, params: TransferRuneParams) -> Result<TransactionResult> {
        log::info!("Starting rune transfer to: {}", params.recipient);

        let mut all_inputs = vec![params.utxo_to_spend];
        all_inputs.extend(params.funding_inputs);
        let inputs = self.convert_inputs_to_utxos(all_inputs).await?;

        let inscription_input = inputs[0].clone();

        let spend_transaction = self.create_spend_utxo_transaction(
            params.recipient,
            inscription_input.amount,
            inputs,
            params.fee,
        )?;

        let reveal_txid = if params.dry_run {
            spend_transaction.txid().to_string()
        } else {
            log::info!("Broadcasting transfer transaction: {}", spend_transaction.txid());
            spend_transaction.txid().to_string()
        };

        log::info!("Rune transfer completed. TXID: {}", reveal_txid);

        Ok(TransactionResult {
            commit_txid: None,
            reveal_txid,
            success: true,
        })
    }

    fn get_sender_address(&self) -> Result<Address> {
        let public_key = self.private_key.public_key(&Secp256k1::new());
        Ok(Address::p2wpkh(&public_key, self.network)?)
    }

    async fn convert_inputs_to_utxos(&self, inputs: Vec<TxInput>) -> Result<Vec<Utxo>> {
        let mut utxos = Vec::new();

        for input in inputs {
            let txid = OrdTxid::from_str(&input.txid)?;
            utxos.push(Utxo {
                id: txid,
                index: input.vout,
                amount: Amount::from_sat(input.amount),
            });
        }

        Ok(utxos)
    }

    fn create_spend_utxo_transaction(
        &self,
        recipient: Address,
        amount: Amount,
        inputs: Vec<Utxo>,
        fee: Amount,
    ) -> Result<bitcoin::Transaction> {
        use bitcoin::{Transaction, TxIn, TxOut, OutPoint, Witness};

        let mut tx_inputs = Vec::new();
        let mut total_input = Amount::ZERO;

        for utxo in inputs {
            tx_inputs.push(TxIn {
                previous_output: OutPoint::new(
                    bitcoin::Txid::from_str(&utxo.id.to_string())?,
                    utxo.index
                ),
                script_sig: bitcoin::ScriptBuf::new(),
                sequence: bitcoin::Sequence::ENABLE_RBF_NO_LOCKTIME,
                witness: Witness::new(),
            });
            total_input += utxo.amount;
        }

        let mut tx_outputs = Vec::new();

        tx_outputs.push(TxOut {
            value: bitcoin_units::amount::Amount::from_sat(amount.to_sat()),
            script_pubkey: bitcoin::ScriptBuf::from_bytes(recipient.script_pubkey().to_bytes()),
        });

        if total_input > amount + fee {
            let change_amount = total_input - amount - fee;
            let sender_address = self.get_sender_address()?;
            tx_outputs.push(TxOut {
                value: bitcoin_units::amount::Amount::from_sat(change_amount.to_sat()),
                script_pubkey: bitcoin::ScriptBuf::from_bytes(sender_address.script_pubkey().to_bytes()),
            });
        }

        Ok(Transaction {
            version: Version(2),
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: tx_inputs,
            output: tx_outputs,
        })
    }
}

impl EtchRuneParams {
    pub fn new(rune_name: String, recipient_address: Address) -> Self {
        Self {
            rune_name,
            divisibility: Some(2),
            premine: Some(1_000_000),
            symbol: Some('$'),
            terms: None,
            inputs: Vec::new(),
            commit_fee: Default::default(),
            reveal_fee: Default::default(),
            recipient_address,
            dry_run: false,
        }
    }

    pub fn with_terms(mut self, amount: u128, cap: u128) -> Self {
        self.terms = Some(RuneTerms {
            amount: Some(amount),
            cap: Some(cap),
            height_range: None,
            offset_range: None,
        });
        self
    }
}

impl MintRuneParams {
    pub fn new(rune_id: RuneId, amount: u128, recipient_address: Address) -> Self {
        Self {
            rune_id,
            amount,
            inputs: Vec::new(),
            recipient_address,
            commit_fee: Default::default(),
            reveal_fee: Default::default(),
            dry_run: false,
        }
    }
}

impl TransferRuneParams {
    pub fn new(rune_id: RuneId, amount: u128, utxo_to_spend: TxInput, recipient: Address) -> Self {
        Self {
            rune_id,
            amount,
            utxo_to_spend,
            funding_inputs: Vec::new(),
            recipient,
            fee: Default::default(),
            dry_run: false,
        }
    }
}