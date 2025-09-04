use frost::mocks::MockSignerUserStorage;
use frost::signer::FrostSigner;
use verifier_config_parser::config::SignerConfig;
use std::sync::Arc;

pub fn create_frost_signer(
    signer_config: SignerConfig,
) -> FrostSigner {
    FrostSigner::new(
        Arc::new(MockSignerUserStorage::new()),
        signer_config.identifier,
        signer_config.total_participants,
        signer_config.threshold,
    )
}
