use crate::signer_client::SignerClient;
use crate::tx_checker_client::TxCheckerClient;
use btc_resp_aggregator::aggregator::{BtcAggregatorParams, BtcConfirmationsAggregator};
use btc_resp_aggregator::traits::TxCheckerClientTrait;
use frost::aggregator::FrostAggregator;
use frost::traits::{AggregatorMusigIdStorage, AggregatorSignSessionStorage, SignerClient as SignerClientTrait};
use frost_secp256k1_tr::Identifier;
use gateway_config_parser::config::ServerConfig;
use global_utils::network::convert_to_http_url;
use std::collections::BTreeMap;
use std::sync::Arc;

pub fn create_aggregator_from_config(
    config: ServerConfig,
    musig_id_storage: Arc<dyn AggregatorMusigIdStorage>,
    sign_session_storage: Arc<dyn AggregatorSignSessionStorage>,
) -> anyhow::Result<FrostAggregator> {
    let mut verifiers = BTreeMap::<Identifier, Arc<dyn SignerClientTrait>>::new();

    for verifier in config.verifiers.0 {
        let signer_client = SignerClient::new(verifier.clone());
        verifiers.insert(verifier.id.try_into()?, Arc::new(signer_client));
    }

    Ok(FrostAggregator::new(verifiers, musig_id_storage, sign_session_storage))
}

pub fn create_btc_resp_checker_aggregator_from_config(
    config: ServerConfig,
) -> anyhow::Result<BtcConfirmationsAggregator> {
    let mut verifiers = BTreeMap::<Identifier, Arc<dyn TxCheckerClientTrait>>::new();

    for verifier in config.verifiers.0 {
        let signer_client = TxCheckerClient::new(verifier.clone());
        verifiers.insert(verifier.id.try_into()?, Arc::new(signer_client));
    }

    Ok(BtcConfirmationsAggregator::new(
        verifiers,
        BtcAggregatorParams {
            threshold: config.aggregator.threshold,
            total_participants: config.aggregator.total_participants,
            interval_millisecond: config.aggregator.update_interval_milliseconds,
            bridge_runes_gateway_url: convert_to_http_url(
                config.server_public.get_app_binding_url()?,
                Some(BtcConfirmationsAggregator::RUN_BRIDGE_RUNE_SPARK_FLOW_PATH),
            )?,
        },
    ))
}
