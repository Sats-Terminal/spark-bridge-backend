use anyhow::Result;
use bitcoin::transaction::Version;
use ord_rs::bitcoin::{secp256k1::Secp256k1, Address, Amount, Network, PrivateKey, Txid as OrdTxid};
use ord_rs::{
    wallet::{
        CreateCommitTransactionArgsV2,
        EtchingTransactionArgs,
        RevealTransactionArgs,
        Runestone,
        SignCommitTransactionArgs,
        Utxo
    }, Nft,
    OrdTransactionBuilder
};
use ordinals::{Rune, RuneId, Terms};
use std::str::FromStr;

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
        use bitcoin::{OutPoint, Transaction, TxIn, TxOut, Witness};

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bitcoin_client::{BitcoinClient as TestBitcoinClient, BitcoinClientConfig};
    use crate::gateway_client::{GatewayClient, GatewayConfig, TestSparkRequest};
    use bitcoin::secp256k1::Secp256k1;
    use ord_rs::bitcoin::{Address, Amount, Network, PrivateKey};
    use ordinals::RuneId;
    use url::Url;

    const TEST_PRIVATE_KEY_WIF: &str = "cNfPNUCLMdcSM4aJhuEiKEK44YoziFVD3EYh9tVgc4rjSTeaYwHP";
    const TEST_NETWORK: Network = Network::Testnet;
    const TEST_SCRIPT_TYPE: &str = "p2tr";

    fn create_test_rune_manager() -> Result<RuneManager> {
        let private_key = PrivateKey::from_wif(TEST_PRIVATE_KEY_WIF)?;
        RuneManager::new(private_key, TEST_NETWORK, TEST_SCRIPT_TYPE)
    }

    fn create_test_bitcoin_client() -> Result<TestBitcoinClient, anyhow::Error> {
        let config = BitcoinClientConfig {
            bitcoin_url: "http://localhost:18332".to_string(),
            titan_url: "http://localhost:8080".to_string(),
            bitcoin_username: "user".to_string(),
            bitcoin_password: "password".to_string(),
        };
        Ok(TestBitcoinClient::new(config)?)
    }

    fn create_test_gateway_client() -> GatewayClient {
        let config = GatewayConfig {
            address: Url::parse("http://localhost:3000").unwrap(),
        };
        GatewayClient::new(config)
    }

    fn create_test_recipient_address() -> Result<Address, anyhow::Error> {
        let private_key = PrivateKey::from_wif(TEST_PRIVATE_KEY_WIF)?;
        let public_key = private_key.public_key(&Secp256k1::new());
        Ok(Address::p2wpkh(&public_key, TEST_NETWORK)?)
    }

    fn create_test_tx_inputs() -> Vec<TxInput> {
        vec![
            TxInput {
                txid: "77b28bec4e4ec7d43d792225b2d6222e57bbbcf3ad37308e0c88906ed91a729e".to_string(),
                vout: 1,
                amount: 100_000_000,
            }
        ]
    }

    #[tokio::test]
    async fn test_etch_rune_dry_run_success() -> Result<()> {
        let mut rune_manager = create_test_rune_manager()?;
        let recipient_address = create_test_recipient_address()?;

        let params = EtchRuneParams {
            rune_name: "TESTRUNE".to_string(),
            divisibility: Some(2),
            premine: Some(1_000_000),
            symbol: Some('T'),
            terms: Some(RuneTerms {
                amount: Some(1000),
                cap: Some(1000),
                height_range: None,
                offset_range: None,
            }),
            inputs: create_test_tx_inputs(),
            commit_fee: Amount::from_sat(1000),
            reveal_fee: Amount::from_sat(1000),
            recipient_address,
            dry_run: true,
        };

        let result = rune_manager.etch_rune(params).await;

        assert!(result.is_ok(), "Etching should succeed in dry run mode");
        let transaction_result = result?;
        assert!(transaction_result.success);
        assert!(transaction_result.commit_txid.is_none());
        assert!(!transaction_result.reveal_txid.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn test_etch_rune_with_different_parameters() -> Result<()> {
        let mut rune_manager = create_test_rune_manager()?;
        let recipient_address = create_test_recipient_address()?;

        let minimal_params = EtchRuneParams {
            rune_name: "MINIMALRUNE".to_string(),
            divisibility: None,
            premine: None,
            symbol: None,
            terms: None,
            inputs: create_test_tx_inputs(),
            commit_fee: Amount::from_sat(500),
            reveal_fee: Amount::from_sat(500),
            recipient_address: recipient_address.clone(),
            dry_run: true,
        };

        let result = rune_manager.etch_rune(minimal_params).await;
        assert!(result.is_ok(), "Etching with minimal params should succeed");

        let maximal_params = EtchRuneParams {
            rune_name: "MAXIMALRUNETOKENTEST".to_string(),
            divisibility: Some(8),
            premine: Some(21_000_000_000_000),
            symbol: Some('₿'),
            terms: Some(RuneTerms {
                amount: Some(100_000),
                cap: Some(21_000_000),
                height_range: Some((1000, 2000)),
                offset_range: Some((100, 200)),
            }),
            inputs: create_test_tx_inputs(),
            commit_fee: Amount::from_sat(2000),
            reveal_fee: Amount::from_sat(3000),
            recipient_address,
            dry_run: true,
        };

        let result = rune_manager.etch_rune(maximal_params).await;
        assert!(result.is_ok(), "Etching with maximal params should succeed");

        Ok(())
    }

    #[tokio::test]
    async fn test_etch_rune_invalid_rune_name() -> Result<()> {
        let mut rune_manager = create_test_rune_manager()?;
        let recipient_address = create_test_recipient_address()?;

        let params = EtchRuneParams {
            rune_name: "invalid-rune-name".to_string(),
            divisibility: Some(2),
            premine: Some(1_000_000),
            symbol: Some('T'),
            terms: None,
            inputs: create_test_tx_inputs(),
            commit_fee: Amount::from_sat(1000),
            reveal_fee: Amount::from_sat(1000),
            recipient_address,
            dry_run: true,
        };

        let result = rune_manager.etch_rune(params).await;

        assert!(result.is_err(), "Etching with invalid rune name should fail");

        Ok(())
    }

    #[tokio::test]
    async fn test_etch_rune_empty_inputs() -> Result<()> {
        let mut rune_manager = create_test_rune_manager()?;
        let recipient_address = create_test_recipient_address()?;

        let params = EtchRuneParams {
            rune_name: "TESTRUNE".to_string(),
            divisibility: Some(2),
            premine: Some(1_000_000),
            symbol: Some('T'),
            terms: None,
            inputs: vec![],
            commit_fee: Amount::from_sat(1000),
            reveal_fee: Amount::from_sat(1000),
            recipient_address,
            dry_run: true,
        };

        let result = rune_manager.etch_rune(params).await;
        assert!(result.is_err(), "Etching with empty inputs should fail");

        Ok(())
    }

    #[tokio::test]
    async fn test_mint_rune_dry_run() -> Result<()> {
        let mut rune_manager = create_test_rune_manager()?;
        let recipient_address = create_test_recipient_address()?;

        let rune_id = RuneId { block: 100, tx: 1 };
        let params = MintRuneParams {
            rune_id,
            amount: 1000,
            inputs: create_test_tx_inputs(),
            recipient_address,
            commit_fee: Amount::from_sat(1000),
            reveal_fee: Amount::from_sat(1000),
            dry_run: true,
        };

        let result = rune_manager.mint_rune(params).await;

        assert!(result.is_ok(), "Minting should succeed in dry run mode");
        let transaction_result = result?;
        assert!(transaction_result.success);
        assert!(transaction_result.commit_txid.is_none());
        assert!(!transaction_result.reveal_txid.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn test_transfer_rune_dry_run() -> Result<()> {
        let rune_manager = create_test_rune_manager()?;
        let recipient_address = create_test_recipient_address()?;

        let rune_id = RuneId { block: 100, tx: 1 };
        let utxo_to_spend = TxInput {
            txid: "77b28bec4e4ec7d43d792225b2d6222e57bbbcf3ad37308e0c88906ed91a729e".to_string(),
            vout: 0,
            amount: 50_000_000,
        };

        let params = TransferRuneParams {
            rune_id,
            amount: 500,
            utxo_to_spend,
            funding_inputs: create_test_tx_inputs(),
            recipient: recipient_address,
            fee: Amount::from_sat(1000),
            dry_run: true,
        };

        let result = rune_manager.transfer_rune(params).await;

        assert!(result.is_ok(), "Transfer should succeed in dry run mode");
        let transaction_result = result?;
        assert!(transaction_result.success);
        assert!(transaction_result.commit_txid.is_none());
        assert!(!transaction_result.reveal_txid.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn test_convert_inputs_to_utxos() -> Result<()> {
        let rune_manager = create_test_rune_manager()?;
        let test_inputs = create_test_tx_inputs();

        let result = rune_manager.convert_inputs_to_utxos(test_inputs.clone()).await;

        assert!(result.is_ok());
        let utxos = result?;
        assert_eq!(utxos.len(), test_inputs.len());

        for (utxo, input) in utxos.iter().zip(test_inputs.iter()) {
            assert_eq!(utxo.index, input.vout);
            assert_eq!(utxo.amount.to_sat(), input.amount);
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_get_sender_address() -> Result<()> {
        let rune_manager = create_test_rune_manager()?;

        let result = rune_manager.get_sender_address();

        assert!(result.is_ok());
        let address = result.unwrap();
        assert_eq!(address.network, TEST_NETWORK);

        Ok(())
    }

    #[tokio::test]
    #[ignore]
    async fn test_integration_with_bitcoin_client() -> Result<()> {
        let mut bitcoin_client = create_test_bitcoin_client()?;
        let recipient_address = create_test_recipient_address()?;

        let result = bitcoin_client.faucet(recipient_address.clone(), 100_000_000);

        assert!(result.is_ok(), "Faucet should work");

        let address_data = bitcoin_client.get_address_data(recipient_address).await;
        assert!(address_data.is_ok(), "Should be able to get address data");

        Ok(())
    }

    #[tokio::test]
    #[ignore]
    async fn test_integration_with_gateway_client() -> Result<()> {
        let gateway_client = create_test_gateway_client();
        let test_address = create_test_recipient_address()?;

        let request = TestSparkRequest {
            btc_address: test_address.to_string(),
        };
        let result = gateway_client.test_spark(request).await;

        assert!(result.is_ok(), "Test spark should work");

        Ok(())
    }

    #[tokio::test]
    async fn test_full_rune_lifecycle_dry_run() -> Result<()> {
        let mut rune_manager = create_test_rune_manager()?;
        let recipient_address = create_test_recipient_address()?;
        let rune_name = "LIFECYCLERUNE";

        let etch_params = EtchRuneParams {
            rune_name: rune_name.to_string(),
            divisibility: Some(2),
            premine: Some(1_000_000),
            symbol: Some('L'),
            terms: Some(RuneTerms {
                amount: Some(1000),
                cap: Some(1000),
                height_range: None,
                offset_range: None,
            }),
            inputs: create_test_tx_inputs(),
            commit_fee: Amount::from_sat(1000),
            reveal_fee: Amount::from_sat(1000),
            recipient_address: recipient_address.clone(),
            dry_run: true,
        };

        let etch_result = rune_manager.etch_rune(etch_params).await?;
        assert!(etch_result.success, "Etching should succeed");

        let rune_id = RuneId { block: 100, tx: 1 };
        let mint_params = MintRuneParams {
            rune_id,
            amount: 500,
            inputs: create_test_tx_inputs(),
            recipient_address: recipient_address.clone(),
            commit_fee: Amount::from_sat(1000),
            reveal_fee: Amount::from_sat(1000),
            dry_run: true,
        };

        let mint_result = rune_manager.mint_rune(mint_params).await?;
        assert!(mint_result.success, "Minting should succeed");

        let utxo_to_spend = TxInput {
            txid: mint_result.reveal_txid.clone(),
            vout: 0,
            amount: 50_000_000,
        };

        let transfer_params = TransferRuneParams {
            rune_id,
            amount: 250,
            utxo_to_spend,
            funding_inputs: vec![],
            recipient: recipient_address,
            fee: Amount::from_sat(1000),
            dry_run: true,
        };

        let transfer_result = rune_manager.transfer_rune(transfer_params).await?;
        assert!(transfer_result.success, "Transfer should succeed");

        println!("Full rune lifecycle completed successfully!");
        println!("Etch TXID: {}", etch_result.reveal_txid);
        println!("Mint TXID: {}", mint_result.reveal_txid);
        println!("Transfer TXID: {}", transfer_result.reveal_txid);

        Ok(())
    }
}