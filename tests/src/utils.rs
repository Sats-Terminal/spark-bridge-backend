use crate::error::RuneError;
use bitcoin::Transaction;
use bitcoin::key::TapTweak;
use bitcoin::secp256k1::Message;
use bitcoin::sighash::{Prevouts, SighashCache, TapSighashType};
use bitcoin::taproot::Signature as TaprootSignature;
use bitcoin::{
    Address, Network,
    key::UntweakedPublicKey,
    secp256k1::{Keypair, Secp256k1},
};
use bitcoin::{Amount, TxOut, Witness};
use rand_core::OsRng;

pub fn create_credentials() -> (Address, Keypair) {
    let secp = Secp256k1::new();
    let mut rng = OsRng;
    let network = Network::Regtest;
    let keypair = Keypair::new(&secp, &mut rng);
    let untweaked_public_key = UntweakedPublicKey::from_keypair(&keypair).0;
    let p2tr_address = Address::p2tr(&secp, untweaked_public_key, None, network);
    (p2tr_address, keypair)
}

pub fn sign_transaction(
    transaction: &mut Transaction,
    prev_input_amounts: Vec<u64>,
    p2tr_address: Address,
    keypair: Keypair,
) -> Result<(), RuneError> {
    let mut sighash_cache = SighashCache::new(transaction.clone());

    if prev_input_amounts.len() != transaction.input.len() {
        return Err(RuneError::SignTransactionError(format!(
            "Invalid number of input amounts: {} != {}",
            prev_input_amounts.len(),
            transaction.input.len()
        )));
    }

    let txouts = prev_input_amounts
        .iter()
        .map(|o| TxOut {
            value: Amount::from_sat(*o),
            script_pubkey: p2tr_address.script_pubkey(),
        })
        .collect::<Vec<_>>();

    let mut messages = vec![];
    for i in 0..transaction.input.len() {
        let message_hash = sighash_cache
            .taproot_key_spend_signature_hash(i, &Prevouts::All(&txouts), TapSighashType::All)
            .map_err(|e| RuneError::SignTransactionError(format!("Failed to create message hash: {}", e)))?;
        let message = Message::try_from(message_hash)
            .map_err(|e| RuneError::SignTransactionError(format!("Failed to create message: {}", e)))?;
        messages.push(message);
    }

    let tweaked_keypair = keypair.tap_tweak(&Secp256k1::new(), None).to_keypair();

    for (i, message) in messages.iter().enumerate() {
        let signature = Secp256k1::new().sign_schnorr_no_aux_rand(&message, &tweaked_keypair);

        let taproot_signature = TaprootSignature {
            signature,
            sighash_type: TapSighashType::All,
        };

        transaction.input[i].witness = Witness::p2tr_key_spend(&taproot_signature);
    }

    Ok(())
}
