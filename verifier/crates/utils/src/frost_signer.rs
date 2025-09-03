use frost::config::SignerConfig;
use frost::mocks::MockSignerUserStorage;
use frost::signer::FrostSigner;
use std::sync::Arc;

pub fn create_frost_signer(config: SignerConfig) -> FrostSigner {
    FrostSigner::new(config, Arc::new(MockSignerUserStorage::new()))
}
