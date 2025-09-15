use btc_resp_aggregator::error::BtcTxCheckerError;
use btc_resp_aggregator::traits::BtcTxIdStatusStorage;
use btc_resp_aggregator::tx_checker::BtcTxChecker;
use frost::signer::FrostSigner;
use frost::traits::{SignerMusigIdStorage, SignerSignSessionStorage};
use std::net::IpAddr;
use std::sync::Arc;
use verifier_config_parser::config::{BtcIndexerConfig, SignerConfig};

pub fn create_frost_signer(
    signer_config: SignerConfig,
    musig_id_storage: Arc<dyn SignerMusigIdStorage>,
    sign_session_storage: Arc<dyn SignerSignSessionStorage>,
) -> FrostSigner {
    FrostSigner::new(
        signer_config.identifier,
        musig_id_storage,
        sign_session_storage,
        signer_config.total_participants,
        signer_config.threshold,
    )
}

pub fn create_btc_resp_aggregator(
    signer_config: SignerConfig,
    btc_indexer_config: BtcIndexerConfig,
    tx_id_storage: Arc<dyn BtcTxIdStatusStorage>,
    verifier_addr: (IpAddr, u16),
) -> Result<BtcTxChecker, BtcTxCheckerError> {
    BtcTxChecker::new(
        signer_config.identifier.try_into().unwrap(),
        signer_config.total_participants,
        signer_config.threshold,
        verifier_addr,
        btc_indexer_config.address,
        tx_id_storage,
    )
}
