use bitcoin::{Transaction, TxIn, OutPoint, ScriptBuf, Sequence, Witness, Address, TxOut, Amount};
use bitcoin::sighash::{SighashCache, TapSighashType, Prevouts};
use bitcoin::hashes::Hash;
use bitcoin::transaction::Version;
use bitcoin::secp256k1::SecretKey;
use bitcoin::secp256k1::Message;
use bitcoin::secp256k1::schnorr::Signature;
use bitcoin::taproot::Signature as TaprootSignature;
use bitcoin::secp256k1::Secp256k1;
use bitcoin::secp256k1::Keypair;
use bitcoin::key::TapTweak;
use crate::errors::RuneTransferError;
use ordinals::{Edict, RuneId, Runestone};


pub fn create_rune_transfer(
    outputs_to_spend: Vec<OutPoint>,
    output_addresses: Vec<Address>,
    output_sats_amounts: Vec<u64>,
    output_runes_amounts: Vec<u128>,
    rune_id: RuneId,
) -> Result<Transaction, RuneTransferError> {
    if output_sats_amounts.len() != output_runes_amounts.len() {
        return Err(RuneTransferError::InvalidData("outputs_to_spend and output_addresses must have the same length".to_string()));
    }

    let mut inputs = Vec::new();
    for output in outputs_to_spend {
        inputs.push(TxIn {
            previous_output: OutPoint::new(output.txid, output.vout),
            script_sig: ScriptBuf::new(),
            sequence: Sequence::ENABLE_RBF_NO_LOCKTIME, 
            witness: Witness::new(),
        });
    }

    let mut edicts = Vec::new();

    for (i, runes_amount) in output_runes_amounts.iter().enumerate() {
        if *runes_amount > 0 {
            edicts.push(Edict {
                id: rune_id,
                amount: *runes_amount,
                output: 1 + i as u32,
            });
        }
    }

    let runestone = Runestone {
        edicts: edicts,
        etching: None,
        mint: None,
        pointer: None,
    };

    let op_return_script: ScriptBuf = runestone.encipher();

    let mut outputs = Vec::new();
    outputs.push(TxOut {
        value: Amount::from_sat(0),
        script_pubkey: op_return_script,
    });
    for (output_sats_amount, output_address) in output_sats_amounts.iter().zip(output_addresses.iter()) {
        outputs.push(TxOut {
            value: Amount::from_sat(*output_sats_amount),
            script_pubkey: output_address.script_pubkey(),
        });
    }

    let transation = Transaction {
        version: Version::TWO,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: inputs,
        output: outputs,
    };

    Ok(transation)
}

pub fn create_message_hash(
    transaction: &Transaction, 
    previous_output_addresses: &Vec<Address>, 
    previous_output_sats_amounts: &Vec<u64>, 
    input_index: usize
) -> Result<[u8; 32], RuneTransferError> {
    if previous_output_addresses.len() != previous_output_sats_amounts.len() {
        return Err(RuneTransferError::InvalidData("previous_output_addresses and previous_output_sats_amounts must have the same length".to_string()));
    }

    let previous_outputs = previous_output_addresses.iter().zip(previous_output_sats_amounts.iter()).map(|(address, amount)| TxOut {
        value: Amount::from_sat(*amount),
        script_pubkey: address.script_pubkey(),
    }).collect::<Vec<TxOut>>();

    let mut sighash_cache = SighashCache::new(transaction);
    let message_hash = sighash_cache.taproot_key_spend_signature_hash(
        input_index,
        &Prevouts::All(&previous_outputs),
        TapSighashType::All,
    ).map_err(|e| RuneTransferError::HashError(format!("Failed to create message hash: {}", e)))?;

    let byte_array = message_hash.to_raw_hash().to_byte_array();
    Ok(byte_array)
}

pub fn sign_message_hash(
    message_hash: [u8; 32],
    secret_key: SecretKey,
) -> Signature{
    let ctx = Secp256k1::new();
    let message = Message::from_digest(message_hash);
    let keypair = Keypair::from_secret_key(&ctx, &secret_key);
    let tweaked_keypair = keypair.tap_tweak(&ctx, None);
    let signature = ctx.sign_schnorr_no_aux_rand(&message, &tweaked_keypair.to_keypair());
    signature
}

pub fn add_signature_to_transaction(
    transaction: &mut Transaction,
    input_index: usize,
    signature: Signature,
) {
    let taproot_signature = TaprootSignature {
        signature: signature,
        sighash_type: TapSighashType::All,
    };
    transaction.input[input_index].witness = Witness::p2tr_key_spend(&taproot_signature);
}
