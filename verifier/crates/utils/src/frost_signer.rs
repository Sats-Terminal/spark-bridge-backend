use frost::signer::FrostSigner;
use frost::traits::{SignerMusigIdStorage, SignerSignSessionStorage};
use std::sync::Arc;
use verifier_config_parser::config::SignerConfig;

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
