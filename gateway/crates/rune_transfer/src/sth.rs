use anyhow::Result;
use bitcoin::{
  Address, Amount, Network, ScriptBuf, Transaction, TxIn, TxOut, Txid, TxVersion,
  blockdata::script::Builder, consensus::encode::serialize, sequence::Sequence, OutPoint,
};
use ord_rs::wallet::{LocalSigner, Wallet, Utxo};
use ordinals::{Runestone, Edict, RuneId};

fn main() -> Result<()> {
  // ---- 0) Network & keys
  let network = Network::Testnet;
  let wif = std::env::var("WIF")?;                         // sender’s key (taproot or p2wpkh)
  let signer = LocalSigner::new(bitcoin::PrivateKey::from_wif(&wif)?);
  let wallet = Wallet::new(network, Box::new(signer));     // minimal wallet wrapper from ord-rs

  // ---- 1) Inputs: you MUST spend the UTXO that carries the rune balance
  // Replace these with the actual UTXO (txid:vout) that holds your runes
  let rune_input = Utxo {
    id: Txid::from_str_radix("aaaaaaaa...deadbeef", 16)?,  // txid hex
    index: 1,
    amount: Amount::from_sat(1200),                        // sats in that UTXO (not the rune amt)
  };

  // (Optional) fee-funding inputs if the rune-carrying UTXO is small:
  let extra_inputs: Vec<Utxo> = vec![];

  // ---- 2) Outputs: where the runes will land
  // Output #0 will be OP_RETURN (runestone). Output #1 must be a spendable address for the recipient.
  let recipient = Address::from_str("tb1p....recipient....")?;
  let recipient_value = Amount::from_sat(546);             // keep above dust; adjust for your script type

  // ---- 3) Build the Runestone for a TRANSFER
  // RuneId "BLOCK:TX" where the rune was etched, e.g. 840000:12
  let rune_id = RuneId::new(840_000, 12).expect("valid id");

  // Send 1_000_000 units of this rune to output index 1
  let edict = Edict { id: rune_id, amount: 1_000_000u128, output: 1 };

  // Pointer says where any unallocated remainder goes (we’ll point to #1 as well)
  let runestone = Runestone { edicts: vec![edict], etching: None, mint: None, pointer: Some(1) };

  // Encipher to a script: this returns OP_RETURN OP_13 <payload...> per spec
  let op_return_script: ScriptBuf = runestone.encipher();  // ready to drop into an output
  // (This guarantees the leading OP_13 tag and valid push-only payload.) :contentReference[oaicite:1]{index=1}

  // ---- 4) Assemble the Bitcoin transaction
  // Inputs
  let mut vin: Vec<TxIn> = Vec::new();
  vin.push(TxIn {
    previous_output: OutPoint::new(rune_input.id, rune_input.index),
    script_sig: ScriptBuf::new(),
    sequence: Sequence::ENABLE_RBF_NO_LOCKTIME, witness: bitcoin::Witness::new(),
  });
  for utxo in &extra_inputs {
    vin.push(TxIn {
      previous_output: OutPoint::new(utxo.id, utxo.index),
      script_sig: ScriptBuf::new(),
      sequence: Sequence::ENABLE_RBF_NO_LOCKTIME, witness: bitcoin::Witness::new(),
    });
  }

  // Outputs (index 0: Runestone; index 1: recipient; optionally index 2: change)
  let mut vout: Vec<TxOut> = Vec::new();
  vout.push(TxOut { value: Amount::from_sat(0), script_pubkey: op_return_script });  // OP_RETURN
  vout.push(TxOut { value: recipient_value, script_pubkey: recipient.script_pubkey() });

  // Quick & dirty fee calc (replace with a real estimator!)
  let input_sum = rune_input.amount.to_sat()
    + extra_inputs.iter().map(|u| u.amount.to_sat()).sum::<u64>();
  let provisional_fee = 500; // sat (placeholder)
  let output_sum_now = vout.iter().map(|o| o.value.to_sat()).sum::<u64>();
  let change = input_sum.checked_sub(output_sum_now + provisional_fee)
    .ok_or_else(|| anyhow::anyhow!("not enough funds for fee"))?;

  if change > 0 {
    let change_addr = wallet.derive_address(None)?;        // send change back to yourself
    vout.push(TxOut { value: Amount::from_sat(change), script_pubkey: change_addr.script_pubkey() });
  }

  let mut tx = Transaction {
    version: TxVersion::TWO,
    lock_time: bitcoin::absolute::LockTime::ZERO,
    input: vin,
    output: vout,
  };

  // ---- 5) Sign inputs
  // ord-rs exposes a signer trait; here we just call wallet.sign_psbt_or_legacy()
  // but to keep this example focused, we show a simplified call.
  // In real code, build a PSBT and let `wallet` sign it (Taproot preferred).
  let signed = wallet.sign_legacy_or_taproot(&mut tx, None)?; // pseudo: check your ord-rs version API

  // ---- 6) Broadcast using your own RPC or a push-tx API
  let raw = hex::encode(serialize(&signed));
  println!("hex={}", raw);
  Ok(())
}