use crate::signer_client::SignerClient;
use frost::aggregator::FrostAggregator;
use frost::mocks::MockAggregatorMusigIdStorage;
use frost::mocks::MockAggregatorSignSessionStorage;
use frost::traits::SignerClient as SignerClientTrait;
use frost_secp256k1_tr::Identifier;
use gateway_config_parser::config::ServerConfig;
use std::collections::BTreeMap;
use std::sync::Arc;

pub fn create_aggregator_from_config(config: ServerConfig) -> FrostAggregator {
    let mut verifiers = BTreeMap::<Identifier, Arc<dyn SignerClientTrait>>::new();

    for verifier in config.verifiers.0 {
        let signer_client = SignerClient::new(verifier.clone());
        verifiers.insert(verifier.id.try_into().unwrap(), Arc::new(signer_client));
    }

    FrostAggregator::new(
        verifiers,
        Arc::new(MockAggregatorMusigIdStorage::new()),
        Arc::new(MockAggregatorSignSessionStorage::new()),
    )
}
