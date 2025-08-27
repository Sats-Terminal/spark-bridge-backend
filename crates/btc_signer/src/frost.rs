use async_trait::async_trait;
use std::collections::{BTreeMap, HashMap};

use frost::keys::{IdentifierList, KeyPackage as FrostKeyPackage, PublicKeyPackage as FrostPubkeyPackage};
use frost::round1::SigningNonces;
use frost::round2::SignatureShare as FrostSignatureShare;
use frost_secp256k1 as frost;
use frost_secp256k1::Identifier;
use k256::elliptic_curve::sec1::{FromEncodedPoint, ToEncodedPoint};
use k256::EncodedPoint as K256EncodedPoint;
use serde::Serialize;
use sha2::Digest;

use crate::{
    api::Signer,
    config::SignerConfig,
    errors::{Result, SignerError},
    types::*,
};

type FrostError = frost::Error;

fn participantid_to_identifier(id: &ParticipantId) -> Result<Identifier> {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(id.as_ref().as_bytes());
    let hash = hasher.finalize();

    let mut b = [0u8; 2];
    b.copy_from_slice(&hash[..2]);
    let mut val = u16::from_be_bytes(b);
    if val == 0 { val = 1; }

    Identifier::try_from(val)
        .map_err(|e| SignerError::Internal(format!("invalid identifier: {e}")))
}

fn k256_pub_to_secp_pub(kpub: &k256::PublicKey) -> Result<secp256k1::PublicKey> {
    let encoded = kpub.to_encoded_point(true);
    secp256k1::PublicKey::from_slice(encoded.as_bytes())
        .map_err(|e| SignerError::Internal(format!("convert pubkey failed: {e}")))
}

fn secp_pub_from_encoded_bytes(bytes: &[u8]) -> Result<secp256k1::PublicKey> {
    secp256k1::PublicKey::from_slice(bytes)
        .map_err(|e| SignerError::Internal(format!("secp pub from bytes failed: {e}")))
}

fn frost_pubpkg_to_local(pubpkg: &FrostPubkeyPackage) -> Result<PublicKeyPackage> {
    let verifying_key = pubpkg.verifying_key();
    let vk_bytes = verifying_key.serialize()
        .map_err(|e| SignerError::Internal(format!("verifying_key serialize failed: {e}")))?;
    let encoded = K256EncodedPoint::from_bytes(&vk_bytes)
        .map_err(|e| SignerError::Internal(format!("Invalid verifying key bytes: {e}")))?;
    let kpub = k256::PublicKey::from_encoded_point(&encoded)
        .into_option()
        .ok_or_else(|| SignerError::Internal("Invalid verifying key encoded point".to_string()))?;
    let group_public_key = k256_pub_to_secp_pub(&kpub)?;

    let mut public_key_shares: HashMap<ParticipantId, secp256k1::PublicKey> = HashMap::new();
    for (ident, vkey) in pubpkg.verifying_shares() {
        let bytes = vkey.serialize()
            .map_err(|e| SignerError::Internal(format!("share serialize failed: {e}")))?;
        let enc = K256EncodedPoint::from_bytes(&bytes)
            .map_err(|e| SignerError::Internal(format!("Invalid share key bytes: {e}")))?;
        let kpub = k256::PublicKey::from_encoded_point(&enc)
            .into_option()
            .ok_or_else(|| SignerError::Internal("Invalid encoded share".to_string()))?;
        let secp_pub = k256_pub_to_secp_pub(&kpub)?;
        let pid = ParticipantId(format!("{:?}", ident));
        public_key_shares.insert(pid, secp_pub);
    }
    
    let total_participants = pubpkg.verifying_shares().len();

    Ok(PublicKeyPackage {
        group_public_key: group_public_key.into(),
        public_key_shares,
        
        threshold: 2, 
        total_participants: total_participants as u32,
    })
}

fn frost_keypackage_to_local(kp: &FrostKeyPackage, participant_id: &ParticipantId) -> Result<KeyShare> {
    let signing_share = kp.signing_share();
    let sk_bytes = signing_share.serialize();
    let secret = secp256k1::SecretKey::from_slice(&sk_bytes)
        .map_err(|e| SignerError::Internal(format!("secret -> secp failed: {e}")))?;

    let vk_bytes = kp.verifying_key().serialize()
        .map_err(|e| SignerError::Internal(format!("verifying_key serialize failed: {e}")))?;
    let enc = K256EncodedPoint::from_bytes(&vk_bytes)
        .map_err(|e| SignerError::Internal(format!("invalid verifying bytes: {e}")))?;
    let kpub = k256::PublicKey::from_encoded_point(&enc)
        .into_option()
        .ok_or_else(|| SignerError::Internal("invalid encoded verifying".to_string()))?;
    let public_key = k256_pub_to_secp_pub(&kpub)?;
    
    let identifier = kp.identifier();

    Ok(KeyShare {
        participant_id: participant_id.clone(),
        secret_share: secret,
        public_key_share: public_key,
        
        threshold: 2, // !!!
        total_participants: 3, // !!!
    })
}

pub struct FrostSigner {
    config: SignerConfig,
    key_package: Option<FrostKeyPackage>,
    public_key_package: Option<FrostPubkeyPackage>,
    last_signing_nonces: Option<SigningNonces>,
    threshold: u16, 
    total_participants: u16,
}

impl FrostSigner {
    pub fn new(config: SignerConfig) -> Self {
        Self {
            config,
            key_package: None,
            public_key_package: None,
            last_signing_nonces: None,
            threshold: 2,
            total_participants: 3,
        }
    }
    
    pub fn set_parameters(&mut self, threshold: u16, total_participants: u16) {
        self.threshold = threshold;
        self.total_participants = total_participants;
    }
    
    pub fn generate_keys_with_trusted_dealer<R: rand_core::RngCore + rand_core::CryptoRng>(
        participants: &[ParticipantId],
        threshold: u16,
        rng: &mut R,
    ) -> Result<BTreeMap<ParticipantId, KeyShare>> {
        let max_signers = participants.len() as u16;
        let min_signers = threshold;

        let (shares_map, _frost_pubpkg) = frost::keys::generate_with_dealer(
            max_signers,
            min_signers,
            IdentifierList::Default,
            rng,
        ).map_err(|e| SignerError::Internal(format!("frost dealer gen failed: {e}")))?;

        let mut res: BTreeMap<ParticipantId, KeyShare> = BTreeMap::new();

        for (idx, (_ident, secret_share)) in shares_map.into_iter().enumerate() {
            let pid = participants.get(idx)
                .cloned()
                .unwrap_or_else(|| ParticipantId(format!("{}", idx + 1)));

            let kp = FrostKeyPackage::try_from(secret_share)
                .map_err(|e| SignerError::Internal(format!("kp try_from failed: {e}")))?;

            let mut local_keyshare = frost_keypackage_to_local(&kp, &pid)?;
            
            local_keyshare.threshold = threshold as u32;
            local_keyshare.total_participants = max_signers as u32;

            res.insert(pid, local_keyshare);
        }

        Ok(res)
    }
}

#[async_trait]
impl Signer for FrostSigner {
    fn get_participant_id(&self) -> &ParticipantId {
        &self.config.participant_id
    }

    async fn get_public_key_share(&self) -> Result<secp256k1::PublicKey> {
        if let Some(kp) = &self.key_package {
            let vk_bytes = kp.verifying_key().serialize()
                .map_err(|e| SignerError::Internal(format!("verifying_key serialize failed: {e}")))?;
            let enc = K256EncodedPoint::from_bytes(&vk_bytes)
                .map_err(|e| SignerError::Internal(format!("invalid verifying bytes: {e}")))?;
            let kpub = k256::PublicKey::from_encoded_point(&enc)
                .into_option()
                .ok_or_else(|| SignerError::Internal("invalid encoded verifying".to_string()))?;
            k256_pub_to_secp_pub(&kpub)
        } else {
            Err(SignerError::Internal("Key package not initialized".into()))
        }
    }

    async fn dkg_round_1(&mut self) -> Result<DkgRound1Package> {
        let kp = self.key_package.as_ref()
            .ok_or_else(|| SignerError::Internal("KeyPackage not initialized for round1".to_string()))?;
        let mut rng = rand::rngs::OsRng;
        
        let (nonces, commitments) = frost::round1::commit(kp.signing_share(), &mut rng);

        self.last_signing_nonces = Some(nonces);

        let mut commitments_vec = Vec::new();

        let hiding_commitment = commitments.hiding();
        let h_bytes = hiding_commitment.serialize()
            .map_err(|e| SignerError::Internal(format!("failed to serialize hiding commitment: {e}")))?;

        let enc = K256EncodedPoint::from_bytes(&h_bytes)
            .map_err(|e| SignerError::Internal(format!("invalid hiding bytes: {e}")))?;
        let kpub = k256::PublicKey::from_encoded_point(&enc)
            .into_option()
            .ok_or_else(|| SignerError::Internal("invalid hiding encoded".to_string()))?;
        let secp_pub = k256_pub_to_secp_pub(&kpub)?;
        commitments_vec.push(secp_pub);

        Ok(DkgRound1Package {
            participant_id: self.config.participant_id.clone(),
            commitments: commitments_vec,
        })
    }

    async fn dkg_round_2(&mut self, _round1_packages: &[DkgRound1Package]) -> Result<DkgRound2Package> {
        Err(SignerError::Internal(
            "Interactive distributed DKG not implemented".to_string()
        ))
    }

    async fn finalize_dkg(
        &mut self,
        _round1_packages: &[DkgRound1Package],
        _round2_packages: &[DkgRound2Package],
    ) -> Result<(KeyShare, PublicKeyPackage)> {
        let kp = self.key_package.as_ref().ok_or_else(|| SignerError::Internal("Key package not set".to_string()))?;
        let pubpkg = self.public_key_package.as_ref().ok_or_else(|| SignerError::Internal("PublicKeyPackage not set".to_string()))?;

        let mut keyshare = frost_keypackage_to_local(kp, &self.config.participant_id)?;
        
        keyshare.threshold = self.threshold as u32;
        keyshare.total_participants = self.total_participants as u32;

        let mut local_pubpkg = frost_pubpkg_to_local(pubpkg)?;
        local_pubpkg.threshold = self.threshold as u32;
        local_pubpkg.total_participants = self.total_participants as u32;

        Ok((keyshare, local_pubpkg))
    }

    async fn generate_nonce_share(&mut self) -> Result<NonceShare> {
        let kp = self.key_package.as_ref()
            .ok_or_else(|| SignerError::Internal("KeyPackage not initialized".to_string()))?;

        let mut rng = rand::rngs::OsRng;
        let (nonces, commitments) = frost::round1::commit(kp.signing_share(), &mut rng);

        self.last_signing_nonces = Some(nonces);

        let hiding_point_bytes = commitments
            .hiding()
            .serialize()
            .map_err(|e| SignerError::Internal(format!("failed to serialize hiding commitment: {e}")))?;

        let binding_point_bytes = commitments
            .binding()
            .serialize()
            .map_err(|e| SignerError::Internal(format!("failed to serialize binding commitment: {e}")))?;

        let hiding_k256 = k256::PublicKey::from_sec1_bytes(&hiding_point_bytes)
            .map_err(|e| SignerError::Internal(format!("invalid hiding key: {e}")))?;
        let binding_k256 = k256::PublicKey::from_sec1_bytes(&binding_point_bytes)
            .map_err(|e| SignerError::Internal(format!("invalid binding key: {e}")))?;

        let hiding = k256_pub_to_secp_pub(&hiding_k256)?;
        let binding = k256_pub_to_secp_pub(&binding_k256)?;

        Ok(NonceShare {
            participant_id: self.config.participant_id.clone(),
            commitment: NonceCommitment {
                hiding_commitment: hiding,
                binding_commitment: binding,
            },
            nonce: None,
        })
    }

    async fn create_partial_signature(&self, signing_package: &SigningPackage) -> Result<PartialSignature> {
        let kp = self.key_package.as_ref().ok_or_else(|| SignerError::Internal("KeyPackage missing".to_string()))?;
        let nonces = self.last_signing_nonces.as_ref().ok_or_else(|| SignerError::Internal("Nonces missing".to_string()))?;

        let signing_pkg_frost: frost::SigningPackage = serde_json::from_slice(
            &serde_json::to_vec(&signing_package)
                .map_err(|e| SignerError::Internal(format!("serialize signing package failed: {e}")))?,
        ).map_err(|e| SignerError::Internal(format!("convert signing package -> frost failed: {e}")))?;

        let sig_share: FrostSignatureShare = frost::round2::sign(&signing_pkg_frost, nonces, kp)
            .map_err(|e| SignerError::Internal(format!("frost round2 sign failed: {e}")))?;

        let ss_bytes = sig_share.serialize();
        let scalar = secp256k1::Scalar::from_be_bytes(<[u8; 32]>::try_from(ss_bytes).unwrap())
            .map_err(|_| SignerError::Internal("failed to parse frost signature share into Scalar".to_string()))?;

        Ok(PartialSignature {
            participant_id: self.config.participant_id.clone(),
            signature_share: scalar,
        })
    }

    async fn get_intermediate_values(&self, message: &[u8]) -> Result<Vec<u8>> {
        if let Some(kp) = &self.key_package {
            let vk_bytes = kp.verifying_key().serialize().map_err(|e| SignerError::Internal(format!("verifying_key serialize failed: {e}")))?;
            let mut h = sha2::Sha256::new();
            h.update(&vk_bytes);
            h.update(message);
            Ok(h.finalize().to_vec())
        } else {
            Err(SignerError::Internal("KeyPackage not initialized".to_string()))
        }
    }
}