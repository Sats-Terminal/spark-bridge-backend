use frost::mocks::MockSignerMusigIdStorage;
use frost::mocks::MockSignerSignSessionStorage;
use frost::signer::FrostSigner;
use std::sync::Arc;
use verifier_config_parser::config::SignerConfig;

pub fn create_frost_signer(signer_config: SignerConfig) -> FrostSigner {
    FrostSigner::new(
        signer_config.identifier,
        Arc::new(MockSignerMusigIdStorage::new()),
        Arc::new(MockSignerSignSessionStorage::default()),
        signer_config.total_participants,
        signer_config.threshold,
    )
}
