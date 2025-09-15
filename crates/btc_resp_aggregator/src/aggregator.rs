use crate::traits::TxCheckerClientTrait;
use frost_secp256k1_tr::Identifier;
use std::collections::BTreeMap;
use std::sync::Arc;

pub type BtcVerifiers = BTreeMap<Identifier, Arc<dyn TxCheckerClientTrait>>;

pub struct BtcConfirmationsAggregator {
    verifiers: BtcVerifiers, // TODO: implement btc verifiers
}

impl BtcConfirmationsAggregator {
    pub fn new(verifiers: BtcVerifiers) -> Self {
        Self { verifiers }
    }
}
