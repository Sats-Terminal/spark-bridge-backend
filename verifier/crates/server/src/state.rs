use frost::signer::FrostSigner;

#[derive(Clone)]
pub struct AppState {
    pub frost_signer: FrostSigner,
}
