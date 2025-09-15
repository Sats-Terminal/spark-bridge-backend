use crate::signer_client::SignerClient;
use crate::tx_checker_client::TxCheckerClient;
use btc_resp_aggregator::aggregator::BtcConfirmationsAggregator;
use btc_resp_aggregator::traits::{BtcTxIdStatusStorage, TxCheckerClientTrait};
use frost::aggregator::FrostAggregator;
use frost::traits::{AggregatorMusigIdStorage, AggregatorSignSessionStorage, SignerClient as SignerClientTrait};
use frost_secp256k1_tr::Identifier;
use gateway_config_parser::config::ServerConfig;
use std::collections::BTreeMap;
use std::sync::Arc;

pub fn create_aggregator_from_config(
    config: ServerConfig,
    musig_id_storage: Arc<dyn AggregatorMusigIdStorage>,
    sign_session_storage: Arc<dyn AggregatorSignSessionStorage>,
) -> FrostAggregator {
    let mut verifiers = BTreeMap::<Identifier, Arc<dyn SignerClientTrait>>::new();

    for verifier in config.verifiers.0 {
        let signer_client = SignerClient::new(verifier.clone());
        verifiers.insert(verifier.id.try_into().unwrap(), Arc::new(signer_client));
    }

    FrostAggregator::new(verifiers, musig_id_storage, sign_session_storage)
}

pub fn create_btc_resp_checker_aggregator_from_config(config: ServerConfig) -> BtcConfirmationsAggregator {
    let mut verifiers = BTreeMap::<Identifier, Arc<dyn TxCheckerClientTrait>>::new();

    for verifier in config.verifiers.0 {
        let signer_client = TxCheckerClient::new(verifier.clone());
        verifiers.insert(verifier.id.try_into().unwrap(), Arc::new(signer_client));
    }

    BtcConfirmationsAggregator::new(verifiers)
}
