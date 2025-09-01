use async_trait::async_trait;
use frost::keys::dkg::round1;
use frost::keys::{
    KeyPackage, PublicKeyPackage,
};
use frost::round2::SignatureShare;
use frost::SigningPackage;
use frost_secp256k1 as frost;
use frost_secp256k1::keys::dkg::round2;
use frost_secp256k1::Identifier;
use secp256k1::PublicKey;
use std::collections::BTreeMap;
use frost_secp256k1::round1::SigningNonces;
use crate::errors::Result;

#[async_trait]
pub trait Signer: Send + Sync {
    fn get_participant_id(&self) -> &Identifier;

    fn set_participant_id(&mut self, id: Identifier);

    fn set_total_participants(&mut self, total: u16);

    fn set_threshold(&mut self, threshold: u16);

    async fn get_public_key_share(&self) -> Result<PublicKey>;

    async fn dkg_round_1(&mut self) -> Result<round1::Package>;

    async fn dkg_round_2(
        &mut self,
        round1_packages: &BTreeMap<Identifier, round1::Package>,
    ) -> Result<BTreeMap<Identifier, round2::Package>>;

    async fn finalize_dkg(
        &mut self,
        round1_packages: &BTreeMap<Identifier, round1::Package>,
        round2_packages: &BTreeMap<Identifier, round2::Package>,
    ) -> Result<(KeyPackage, PublicKeyPackage)>;

    async fn generate_nonce_share(&mut self) -> Result<SigningNonces>;

    async fn create_partial_signature(
        &self,
        signing_package: &SigningPackage,
        nonces: &SigningNonces,
        key_package: &KeyPackage,
    ) -> Result<SignatureShare>;

    async fn get_intermediate_values(&self, message: &[u8]) -> Result<Vec<u8>>;


}
