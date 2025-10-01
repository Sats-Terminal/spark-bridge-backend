use crate::errors::RuneTransferError;
use bitcoin::hashes::Hash;
use bitcoin::key::TapTweak;
use bitcoin::secp256k1::Keypair;
use bitcoin::secp256k1::Message;
use bitcoin::secp256k1::Secp256k1;
use bitcoin::secp256k1::SecretKey;
use bitcoin::secp256k1::schnorr::Signature;
use bitcoin::sighash::{Prevouts, SighashCache, TapSighashType};
use bitcoin::taproot::Signature as TaprootSignature;
use bitcoin::transaction::Version;
use bitcoin::{Address, Amount, Network, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Txid, Witness};
use global_utils::conversion::decode_address;
use ordinals::{Edict, RuneId, Runestone};
use std::str::FromStr;

#[derive(Clone, Debug)]
pub struct PayingTransferInput {
    pub txid: Txid,
    pub vout: u32,
    pub btc_exit_address: Address,
    pub sats_amount: u64,
    pub none_anyone_can_pay_signature: Signature,
}

#[derive(Clone, Debug)]
pub struct RuneTransferOutput {
    pub address: Address,
    pub sats_amount: u64,
    pub runes_amount: u64,
}

pub fn create_rune_partial_transaction(
    outputs_to_spend: Vec<OutPoint>,
    paying_input: PayingTransferInput,
    rune_transfer_outputs: Vec<RuneTransferOutput>,
    rune_id: String,
) -> Result<Transaction, RuneTransferError> {
    let rune_id = RuneId::from_str(&rune_id)
        .map_err(|e| RuneTransferError::InvalidData(format!("Failed to parse rune id: {}", e)))?;

    let mut inputs = Vec::new();

    let paying_input = TxIn {
        previous_output: OutPoint::new(paying_input.txid, paying_input.vout),
        script_sig: ScriptBuf::new(),
        sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
        witness: Witness::p2tr_key_spend(&TaprootSignature {
            signature: paying_input.none_anyone_can_pay_signature,
            sighash_type: TapSighashType::NonePlusAnyoneCanPay,
        }),
    };
    inputs.push(paying_input);
    
    for output in outputs_to_spend {
        inputs.push(TxIn {
            previous_output: OutPoint::new(output.txid, output.vout),
            script_sig: ScriptBuf::new(),
            sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
            witness: Witness::new(),
        });
    }

    let mut edicts = Vec::new();

    for (i, transfer_output) in rune_transfer_outputs.iter().enumerate() {
        if transfer_output.runes_amount > 0 {
            edicts.push(Edict {
                id: rune_id,
                amount: transfer_output.runes_amount as u128,
                output: 1 + i as u32,
            });
        }
    }

    let runestone = Runestone {
        edicts,
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
    for transfer_output in rune_transfer_outputs.iter() {
        outputs.push(TxOut {
            value: Amount::from_sat(transfer_output.sats_amount),
            script_pubkey: transfer_output.address.script_pubkey(),
        });
    }

    let transaction = Transaction {
        version: Version::TWO,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: inputs,
        output: outputs,
    };

    Ok(transaction)
}

pub fn create_message_hash(
    transaction: &Transaction,
    previous_output_address: Address,
    previous_output_sats_amount: u64,
    input_index: usize,
) -> Result<[u8; 32], RuneTransferError> {
    let previous_output = TxOut {
        value: Amount::from_sat(previous_output_sats_amount),
        script_pubkey: previous_output_address.script_pubkey(),
    };

    let mut sighash_cache = SighashCache::new(transaction);
    let message_hash = sighash_cache
        .taproot_key_spend_signature_hash(
            input_index,
            &Prevouts::One(input_index, previous_output),
            TapSighashType::AllPlusAnyoneCanPay,
        )
        .map_err(|e| RuneTransferError::HashError(format!("Failed to create message hash: {}", e)))?;

    let byte_array = message_hash.to_raw_hash().to_byte_array();
    Ok(byte_array)
}

pub fn sign_message_hash(message_hash: [u8; 32], secret_key: SecretKey) -> Signature {
    let ctx = Secp256k1::new();
    let message = Message::from_digest(message_hash);
    let keypair = Keypair::from_secret_key(&ctx, &secret_key);
    let tweaked_keypair = keypair.tap_tweak(&ctx, None);
    let signature = ctx.sign_schnorr_no_aux_rand(&message, &tweaked_keypair.to_keypair());
    signature
}

pub fn add_signature_to_transaction(transaction: &mut Transaction, input_index: usize, signature: Signature) {
    let taproot_signature = TaprootSignature {
        signature,
        sighash_type: TapSighashType::All,
    };
    transaction.input[input_index].witness = Witness::p2tr_key_spend(&taproot_signature);
}
