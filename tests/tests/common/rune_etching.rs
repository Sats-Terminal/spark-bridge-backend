use std::{str::FromStr, time::Duration};

use bitcoin::{
    Address, Amount, Network, OutPoint, Transaction, TxIn, TxOut, Txid, Witness, XOnlyPublicKey,
    blockdata::opcodes::all::OP_CHECKSIG,
    hashes::Hash,
    key::{Keypair, TapTweak},
    script::{Builder, PushBytesBuf, ScriptBuf},
    secp256k1::{Message, Secp256k1},
    sighash::{Prevouts, SighashCache, TapSighashType},
    taproot::{LeafVersion, Signature as TaprootSignature, TapLeafHash, TaprootBuilder},
    transaction::Version,
};
use ord::Inscription;
use ordinals::{Etching, Rune, RuneId, Runestone, Terms};
use tokio::time::sleep;
use tracing;

use crate::common::{
    bitcoin_client::BitcoinClient,
    constants::{BLOCKS_TO_GENERATE, DEFAULT_DUST_AMOUNT, DEFAULT_FEE_AMOUNT, DEFAULT_REVEAL_AMOUNT},
    error::RuneError,
};

pub struct EtchRuneParams {
    pub rune_name: String,
    pub cap: u64,
    pub amount: u64,
    pub key_pair: Keypair,
    pub faucet_address: Address,
    pub network: Network,
}

pub async fn etch_rune(params: EtchRuneParams, bitcoin_client: &mut impl BitcoinClient) -> Result<RuneId, RuneError> {
    tracing::info!("Starting etch test");

    let secp = Secp256k1::new();

    // Funding

    tracing::info!("Funding default address");

    let reveal_amount = DEFAULT_REVEAL_AMOUNT;
    let dust_amount = DEFAULT_DUST_AMOUNT;
    let fee_amount = DEFAULT_FEE_AMOUNT;

    let utxos_data = bitcoin_client
        .get_address_data(params.faucet_address.clone())
        .await
        .map_err(|e| RuneError::EtchRuneError(format!("Failed to get address data: {}", e)))?;

    tracing::debug!("address rune utxos {:?}", utxos_data);

    if utxos_data.is_empty() {
        return Err(RuneError::EtchRuneError(
            "Address should have more than output".to_string(),
        ));
    }

    let mut funded_outpoint = None;
    let mut faucet_sats = None;
    for utxo in utxos_data.iter() {
        if !utxo.confirmed {
            return Err(RuneError::EtchRuneError("Address is not confirmed".to_string()));
        }
        if utxo.value >= 100_000 {
            faucet_sats = Some(utxo.value);
            funded_outpoint = Some(OutPoint {
                txid: Txid::from_str(&utxo.txid.to_string()).unwrap(),
                vout: utxo.vout,
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
        pointer: None,
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

    let script_output_address = Address::p2tr_tweaked(taproot_spend_info.output_key(), params.network);

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
        .await
        .map_err(|e| RuneError::EtchRuneError(format!("Failed to broadcast inscription transaction: {}", e)))?;
    bitcoin_client
        .generate_blocks(BLOCKS_TO_GENERATE, None)
        .await
        .map_err(|e| RuneError::EtchRuneError(format!("Failed to generate blocks: {}", e)))?;

    tracing::info!("Inscription transaction broadcasted");

    // Create rune etching transaction

    tracing::info!("Creating rune etching transaction");

    let runestone = Runestone {
        etching: Some(etching),
        edicts: vec![],
        mint: None,
        pointer: None,
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
        .await
        .map_err(|e| RuneError::EtchRuneError(format!("Failed to broadcast etching transaction: {}", e)))?;
    bitcoin_client
        .generate_blocks(BLOCKS_TO_GENERATE, None)
        .await
        .map_err(|e| RuneError::EtchRuneError(format!("Failed to generate blocks: {}", e)))?;
    bitcoin_client.wait_mined(&etching_tx.compute_txid()).await.unwrap();

    tracing::info!("Rune etching transaction broadcasted");

    // Check etching transaction

    tracing::info!("Checking rune etching transaction");
    sleep(Duration::from_secs(30)).await;

    let rune_id = bitcoin_client
        .get_rune_id(&etching_tx.compute_txid())
        .await
        .map_err(|e| RuneError::EtchRuneError(format!("Failed to get rune id: {}", e)))?;
    tracing::info!("rune_id: {:?}", rune_id);

    let mut maybe_rune = None;
    for attempt in 0..30 {
        match bitcoin_client.get_rune(rune_id.to_string()).await {
            Ok(rune) => {
                maybe_rune = Some(rune);
                break;
            }
            Err(e) => {
                tracing::warn!("Retrying to fetch rune metadata (attempt {}): {}", attempt + 1, e);
                sleep(Duration::from_secs(1)).await;
            }
        }
    }
    if let Some(rune) = maybe_rune {
        tracing::info!("rune: {:?}", rune);
    } else {
        tracing::warn!("Rune metadata was not available after retries; continuing anyway");
    }

    Ok(rune_id)
}
