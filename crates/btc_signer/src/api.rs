use async_trait::async_trait;
use btc_signer_types::types::{DkgRound1Package, DkgRound2Package, KeyShare, NonceShare, PartialSignature, ParticipantId, PublicKeyPackage, SigningPackage};
use crate::errors::Result;

#[async_trait]
pub trait Signer: Send + Sync {
    fn get_participant_id(&self) -> &ParticipantId;

    async fn get_public_key_share(&self) -> Result<secp256k1::PublicKey>;

    async fn dkg_round_1(&mut self) -> Result<DkgRound1Package>;

    async fn dkg_round_2(
        &mut self,
        round1_packages: &[DkgRound1Package]
    ) -> Result<DkgRound2Package>;

    async fn finalize_dkg(
        &mut self,
        round1_packages: &[DkgRound1Package],
        round2_packages: &[DkgRound2Package],
    ) -> Result<(KeyShare, PublicKeyPackage)>;

    async fn generate_nonce_share(&mut self) -> Result<NonceShare>;

    async fn create_partial_signature(
        &self,
        signing_package: &SigningPackage,
    ) -> Result<PartialSignature>;

    async fn get_intermediate_values(&self, message: &[u8]) -> Result<Vec<u8>>;
}