use ord::Inscription;
use bitcoin::{key::Keypair, XOnlyPublicKey, key::UntweakedPublicKey};
use bitcoin::secp256k1::Secp256k1;
use ordinals::{Etching, Terms, Rune, Runestone};
use rand_core::{OsRng, RngCore};
use bitcoin::script::{PushBytesBuf, ScriptBuf};
use bitcoin::script::Builder;
use bitcoin::blockdata::opcodes::all::OP_CHECKSIG;
use bitcoin::taproot::{TaprootBuilder, Signature as TaprootSignature, TapLeafHash, LeafVersion};
use bitcoin::key::TapTweak;
use bitcoin::{Network, Address, TxIn, TxOut, Witness, OutPoint, Amount, Txid, Transaction};
use bitcoin::transaction::Version;
use bitcoin::secp256k1::Message;
use bitcoin::sighash::{SighashCache, Prevouts, TapSighashType};
use bitcoin::hashes::Hash;
use std::str::FromStr;
use tokio::time::sleep;
use std::time::Duration;
use tracing;
use tests::bitcoin_client::{BitcoinClient, BitcoinClientConfig};
use global_utils::logger::init_logger;

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

#[tokio::test]
async fn test_etch() {
    let _guard = init_logger();

    tracing::info!("Starting etch test");

    let secp = Secp256k1::new();
    let mut rng = OsRng;
    let network = Network::Regtest;
    let keypair = Keypair::new(&secp, &mut rng);

    let untweaked_public_key = UntweakedPublicKey::from_keypair(&keypair).0;
    let default_address = Address::p2tr(&secp, untweaked_public_key, None, network);

    let mut bitcoin_client = BitcoinClient::new(
        BitcoinClientConfig {
            bitcoin_url: "http://127.0.0.1:18443".to_string(),
            titan_url: "http://127.0.0.1:3030".to_string(),
            bitcoin_username: "bitcoin".to_string(),
            bitcoin_password: "bitcoinpass".to_string(),
        }
    ).expect("bitcoin client should work");

    // Funding 

    tracing::info!("Funding default address");

    let faucet_sats = 1_000_000;
    let reveal_amount = 10_000;
    let dust_amount = 546;
    let fee_amount = 5_000;

    bitcoin_client.faucet(default_address.clone(), faucet_sats).expect("faucet should work");

    sleep(Duration::from_secs(1)).await;

    let address_data = bitcoin_client.get_address_data(default_address.clone()).await.expect("address data should work");

    println!("address_data: {:?}", address_data);

    assert!(address_data.outputs.len() > 0, "should have more than output");
    
    let mut funded_outpoint = None;
    for (index, output) in address_data.outputs.iter().enumerate() {
        assert!(output.status.confirmed, "All outputs should be confirmed");
        if output.value == faucet_sats {
            funded_outpoint = Some(OutPoint {
                txid: Txid::from_str(&output.txid.to_string()).unwrap(),
                vout: index as u32,
            });
            break;
        }
    }

    let funded_outpoint = funded_outpoint.expect("funded outpoint should be found");

    tracing::info!("Wallet funded");

    // Create inscription struct

    tracing::info!("Creating inscription struct");

    let etching = Etching {
        rune: Some(Rune::from_str(&random_rune_name()).unwrap()),
        divisibility: Some(3),
        premine: None,
        spacers: None,
        symbol: Some('$'),
        terms: Some(Terms {
            amount: Some(1000000),
            cap: Some(1000),
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

    let p2tr_pubkey = XOnlyPublicKey::from_keypair(&keypair).0;
    let encoded_public_key = p2tr_pubkey.serialize();
    let push_bytes_buf = PushBytesBuf::from(encoded_public_key);

    let builder = Builder::new();
    let builder = builder.push_slice(push_bytes_buf);
    let builder = builder.push_opcode(OP_CHECKSIG);

    let redeem_script = inscription.append_reveal_script_to_builder(builder).into_script();

    let taproot_spend_info = TaprootBuilder::new()
        .add_leaf(0, redeem_script.clone())
        .expect("adding leaf should work")
        .finalize(&secp, p2tr_pubkey)
        .expect("finalizing taproot should work");

    let script_output_address = Address::p2tr_tweaked(taproot_spend_info.output_key(), network);

    let inscription_return_amount = faucet_sats - reveal_amount - fee_amount;

    let tx_out = vec![
        TxOut {
            value: Amount::from_sat(reveal_amount),
            script_pubkey: script_output_address.script_pubkey(),
        },
        TxOut {
            value: Amount::from_sat(inscription_return_amount),
            script_pubkey: default_address.script_pubkey(),
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
    let sighash = sighash_cache.taproot_key_spend_signature_hash(
        0, 
        &Prevouts::All(&[TxOut {
            value: Amount::from_sat(faucet_sats),
            script_pubkey: default_address.script_pubkey(),
        }]), 
        TapSighashType::All
    ).expect("sighash cache should work");

    let msg = Message::from_digest(sighash.to_byte_array());

    let tweaked_keypair = keypair.tap_tweak(&secp, None).to_keypair();
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

    bitcoin_client.broadcast_transaction(inscription_tx.clone()).expect("broadcast transaction should work");
    bitcoin_client.generate_blocks(6, None).expect("generate blocks should work");

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
            script_pubkey: default_address.script_pubkey(),
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
    let sighash_sig = sighash_cache.taproot_script_spend_signature_hash(
        0,
        &prevouts,
        TapLeafHash::from_script(&redeem_script, LeafVersion::TapScript),
        TapSighashType::Default,
    ).expect("sighash cache should work");

    let msg = Message::from_digest(sighash_sig.to_byte_array());

    let signature = TaprootSignature {
        signature: secp.sign_schnorr_no_aux_rand(&msg, &keypair),
        sighash_type: TapSighashType::Default,
    };
    let mut witness = Witness::new();
    witness.push(signature.to_vec());
    witness.push(redeem_script.as_bytes());
    witness.push(taproot_spend_info.control_block(&(redeem_script.clone(), LeafVersion::TapScript)).unwrap().serialize());
    etching_tx.input[0].witness = witness;

    let etching_txid = etching_tx.compute_txid();

    tracing::info!("etching_txid: {:?}", etching_txid);
    tracing::info!("Rune etching transaction signed");

    // Broadcast etching transaction

    tracing::info!("Broadcasting rune etching transaction");

    bitcoin_client.broadcast_transaction(etching_tx.clone()).expect("broadcast transaction should work");
    bitcoin_client.generate_blocks(6, None).expect("generate blocks should work");

    tracing::info!("Rune etching transaction broadcasted");

    // Check etching transaction

    tracing::info!("Checking rune etching transaction");

    sleep(Duration::from_secs(1)).await;
    
    let rune_id = bitcoin_client.get_rune_id(&etching_tx.compute_txid()).await.expect("get rune id should work");
    tracing::info!("rune_id: {:?}", rune_id);

    let rune = bitcoin_client.get_rune(rune_id.to_string()).await.expect("get rune should work");
    tracing::info!("rune: {:?}", rune);
}
