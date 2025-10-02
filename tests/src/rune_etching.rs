use crate::bitcoin_client::BitcoinClient;
use crate::constants::{BLOCKS_TO_GENERATE, DEFAULT_DUST_AMOUNT, DEFAULT_FEE_AMOUNT, DEFAULT_REVEAL_AMOUNT};
use crate::error::RuneError;
use bitcoin::blockdata::opcodes::all::OP_CHECKSIG;
use bitcoin::hashes::Hash;
use bitcoin::key::TapTweak;
use bitcoin::script::Builder;
use bitcoin::script::{PushBytesBuf, ScriptBuf};
use bitcoin::secp256k1::Message;
use bitcoin::secp256k1::Secp256k1;
use bitcoin::sighash::{Prevouts, SighashCache, TapSighashType};
use bitcoin::taproot::{LeafVersion, Signature as TaprootSignature, TapLeafHash, TaprootBuilder};
use bitcoin::transaction::Version;
use bitcoin::{Address, Amount, Network, OutPoint, Transaction, TxIn, TxOut, Txid, Witness};
use bitcoin::{XOnlyPublicKey, key::Keypair};
use ord::Inscription;
use ordinals::RuneId;
use ordinals::{Etching, Rune, Runestone, Terms};
use std::str::FromStr;
use std::time::Duration;
use tokio::time::sleep;
use tracing;

pub struct EtchRuneParams {
    pub rune_name: String,
    pub cap: u64,
    pub amount: u64,
    pub key_pair: Keypair,
    pub faucet_address: Address,
}

pub async fn etch_rune(params: EtchRuneParams, mut bitcoin_client: BitcoinClient) -> Result<RuneId, RuneError> {
    tracing::info!("Starting etch test");

    let secp = Secp256k1::new();
    let network = Network::Regtest;

    // Funding

    tracing::info!("Funding default address");

    let reveal_amount = DEFAULT_REVEAL_AMOUNT;
    let dust_amount = DEFAULT_DUST_AMOUNT;
    let fee_amount = DEFAULT_FEE_AMOUNT;

    let address_data = bitcoin_client
        .get_address_data(params.faucet_address.clone())
        .await
        .map_err(|e| RuneError::EtchRuneError(format!("Failed to get address data: {}", e)))?;

    tracing::debug!("address_data: {:?}", address_data);

    if address_data.outputs.is_empty() {
        return Err(RuneError::EtchRuneError(
            "Address should have more than output".to_string(),
        ));
    }

    let mut funded_outpoint = None;
    let mut faucet_sats = None;
    for output in address_data.outputs.iter() {
        if !output.status.confirmed {
            return Err(RuneError::EtchRuneError("Address is not confirmed".to_string()));
        }
        if output.value >= 100_000 {
            faucet_sats = Some(output.value);
            funded_outpoint = Some(OutPoint {
                txid: Txid::from_str(&output.txid.to_string()).unwrap(),
                vout: output.vout,
            });
            break;
        }
    }

    let funded_outpoint =
        funded_outpoint.ok_or(RuneError::EtchRuneError("Failed to get funded outpoint".to_string()))?;
    let faucet_sats = faucet_sats.ok_or(RuneError::EtchRuneError(
        "Address should have at least 100_000 satoshis on one of the outputs".to_string(),
    ))?;

    tracing::info!("Wallet funded");

    // Create inscription struct

    tracing::info!("Creating inscription struct");

    let etching = Etching {
        rune: Some(Rune::from_str(&params.rune_name).unwrap()),
        divisibility: Some(3),
        premine: None,
        spacers: None,
        symbol: Some('$'),
        terms: Some(Terms {
            amount: Some(params.amount as u128),
            cap: Some(params.cap as u128),
            height: (None, None),
            offset: (None, None),
        }),
        turbo: true,
    };

    let inscription = Inscription {
        content_type: Some("text/plain;charset=utf-8".as_bytes().to_vec()),
        body: Some(etching.rune.unwrap().to_string().as_bytes().to_vec()),
        rune: Some(etching.rune.unwrap().commitment()),
        pointer: Some(vec![]),
        ..Default::default()
    };

    tracing::info!("Inscription struct created");

    // Create inscription transaction

    tracing::info!("Creating inscription transaction");

    let p2tr_pubkey = XOnlyPublicKey::from_keypair(&params.key_pair).0;
    let encoded_public_key = p2tr_pubkey.serialize();
    let push_bytes_buf = PushBytesBuf::from(encoded_public_key);

    let builder = Builder::new();
    let builder = builder.push_slice(push_bytes_buf);
    let builder = builder.push_opcode(OP_CHECKSIG);

    let redeem_script = inscription.append_reveal_script_to_builder(builder).into_script();

    let taproot_spend_info = TaprootBuilder::new()
        .add_leaf(0, redeem_script.clone())
        .map_err(|e| RuneError::EtchRuneError(format!("Failed to add leaf: {}", e)))?
        .finalize(&secp, p2tr_pubkey)
        .map_err(|_| RuneError::EtchRuneError("Failed to finalize taproot".to_string()))?;

    let script_output_address = Address::p2tr_tweaked(taproot_spend_info.output_key(), network);

    let inscription_return_amount = faucet_sats - reveal_amount - fee_amount;

    let tx_out = vec![
        TxOut {
            value: Amount::from_sat(reveal_amount),
            script_pubkey: script_output_address.script_pubkey(),
        },
        TxOut {
            value: Amount::from_sat(inscription_return_amount),
            script_pubkey: params.faucet_address.script_pubkey(),
        },
    ];

    let tx_in = TxIn {
        previous_output: funded_outpoint,
        script_sig: bitcoin::ScriptBuf::new(),
        sequence: bitcoin::Sequence::ENABLE_RBF_NO_LOCKTIME,
        witness: Witness::new(),
    };

    let mut inscription_tx = Transaction {
        version: Version::TWO,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: vec![tx_in],
        output: tx_out,
    };

    tracing::info!("Inscription transaction created");

    // Sign inscription transaction

    tracing::info!("Signing inscription transaction");

    let mut sighash_cache = SighashCache::new(inscription_tx.clone());
    let sighash = sighash_cache
        .taproot_key_spend_signature_hash(
            0,
            &Prevouts::All(&[TxOut {
                value: Amount::from_sat(faucet_sats),
                script_pubkey: params.faucet_address.script_pubkey(),
            }]),
            TapSighashType::All,
        )
        .map_err(|e| RuneError::EtchRuneError(format!("Failed to create sighash signature: {}", e)))?;

    let msg = Message::from_digest(sighash.to_byte_array());

    let tweaked_keypair = params.key_pair.tap_tweak(&secp, None).to_keypair();
    let witness = Witness::p2tr_key_spend(&TaprootSignature {
        signature: secp.sign_schnorr_no_aux_rand(&msg, &tweaked_keypair),
        sighash_type: TapSighashType::All,
    });
    inscription_tx.input[0].witness = witness;

    let inscription_txid = inscription_tx.compute_txid();

    tracing::info!("inscription_txid: {:?}", inscription_txid);
    tracing::info!("Inscription transaction signed");

    // Broadcast inscription transaction

    tracing::info!("Broadcasting inscription transaction");

    bitcoin_client
        .broadcast_transaction(inscription_tx.clone())
        .map_err(|e| RuneError::EtchRuneError(format!("Failed to broadcast inscription transaction: {}", e)))?;
    bitcoin_client
        .generate_blocks(BLOCKS_TO_GENERATE, None)
        .map_err(|e| RuneError::EtchRuneError(format!("Failed to generate blocks: {}", e)))?;

    tracing::info!("Inscription transaction broadcasted");

    // Create rune etching transaction

    tracing::info!("Creating rune etching transaction");

    let runestone = Runestone {
        etching: Some(etching),
        edicts: vec![],
        mint: None,
        pointer: Some(1),
    };

    let btc_030_script = runestone.encipher();
    let btc_031_script = ScriptBuf::from_bytes(btc_030_script.to_bytes());

    let tx_out = vec![
        TxOut {
            value: Amount::from_sat(0),
            script_pubkey: btc_031_script,
        },
        TxOut {
            value: Amount::from_sat(dust_amount),
            script_pubkey: params.faucet_address.script_pubkey(),
        },
    ];

    let tx_in = vec![TxIn {
        previous_output: OutPoint {
            txid: inscription_tx.compute_txid(),
            vout: 0,
        },
        script_sig: ScriptBuf::new(),
        sequence: bitcoin::Sequence::from_consensus(0xffffffff),
        witness: Witness::new(),
    }];

    let mut etching_tx = Transaction {
        version: Version::TWO,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: tx_in,
        output: tx_out,
    };

    tracing::info!("Rune etching transaction created");

    // Sign etching transaction

    tracing::info!("Signing rune etching transaction");

    let prevouts_array = vec![TxOut {
        value: Amount::from_sat(reveal_amount),
        script_pubkey: script_output_address.script_pubkey(),
    }];
    let prevouts = Prevouts::All(&prevouts_array);

    let mut sighash_cache = SighashCache::new(etching_tx.clone());
    let sighash_sig = sighash_cache
        .taproot_script_spend_signature_hash(
            0,
            &prevouts,
            TapLeafHash::from_script(&redeem_script, LeafVersion::TapScript),
            TapSighashType::Default,
        )
        .map_err(|e| RuneError::EtchRuneError(format!("Failed to create sighash signature: {}", e)))?;

    let msg = Message::from_digest(sighash_sig.to_byte_array());

    let signature = TaprootSignature {
        signature: secp.sign_schnorr_no_aux_rand(&msg, &params.key_pair),
        sighash_type: TapSighashType::Default,
    };
    let mut witness = Witness::new();
    witness.push(signature.to_vec());
    witness.push(redeem_script.as_bytes());
    witness.push(
        taproot_spend_info
            .control_block(&(redeem_script.clone(), LeafVersion::TapScript))
            .unwrap()
            .serialize(),
    );
    etching_tx.input[0].witness = witness;

    let etching_txid = etching_tx.compute_txid();

    tracing::info!("etching_txid: {:?}", etching_txid);
    tracing::info!("Rune etching transaction signed");

    // Broadcast etching transaction

    tracing::info!("Broadcasting rune etching transaction");

    bitcoin_client
        .broadcast_transaction(etching_tx.clone())
        .map_err(|e| RuneError::EtchRuneError(format!("Failed to broadcast etching transaction: {}", e)))?;
    bitcoin_client
        .generate_blocks(BLOCKS_TO_GENERATE, None)
        .map_err(|e| RuneError::EtchRuneError(format!("Failed to generate blocks: {}", e)))?;

    tracing::info!("Rune etching transaction broadcasted");

    // Check etching transaction

    tracing::info!("Checking rune etching transaction");

    sleep(Duration::from_secs(1)).await;

    let rune_id = bitcoin_client
        .get_rune_id(&etching_tx.compute_txid())
        .await
        .map_err(|e| RuneError::EtchRuneError(format!("Failed to get rune id: {}", e)))?;
    tracing::info!("rune_id: {:?}", rune_id);

    let rune = bitcoin_client
        .get_rune(rune_id.to_string())
        .await
        .map_err(|e| RuneError::EtchRuneError(format!("Failed to get rune: {}", e)))?;
    tracing::info!("rune: {:?}", rune);

    Ok(rune_id)
}
