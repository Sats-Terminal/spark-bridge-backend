use titan_types::{AddressTxOut, Transaction};

pub fn compare_address_tx_outs(a: &[AddressTxOut], b: &[AddressTxOut]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter()
        .zip(b.iter())
        .all(|(x, y)| x.value == y.value && x.status == y.status && x.txid == y.txid && x.vout == y.vout)
}

pub fn compare_address_tx(a: &Transaction, b: &Transaction) -> bool {
    a.status == b.status
        && a.txid == b.txid
        && a.input.iter().zip(b.input.iter()).all(|(x, y)| {
            x.witness == y.witness
                && x.sequence == y.sequence
                && x.previous_output == y.previous_output
                && x.script_sig == y.script_sig
        })
        && a.lock_time == b.lock_time
        && a.output == b.output
        && a.weight == b.weight
        && a.size == b.size
        && a.weight == b.weight
        && a.version == b.version
}
