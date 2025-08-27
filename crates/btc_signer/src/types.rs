use serde::{Deserialize, Serialize};
use secp256k1::{PublicKey, SecretKey, Scalar, XOnlyPublicKey};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, PartialOrd)]
#[derive(Eq)]
#[derive(Hash)]
#[derive(Ord)]
pub struct ParticipantId(pub String);

impl From<String> for ParticipantId {
    fn from(s: String) -> Self {
        ParticipantId(s)
    }
}

impl AsRef<str> for ParticipantId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyShare {
    pub participant_id: ParticipantId,
    pub secret_share: SecretKey,
    pub public_key_share: PublicKey,
    pub threshold: u32,
    pub total_participants: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicKeyPackage {
    pub group_public_key: XOnlyPublicKey,
    pub public_key_shares: HashMap<ParticipantId, PublicKey>,
    pub threshold: u32,
    pub total_participants: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NonceCommitment {
    pub hiding_commitment: PublicKey,
    pub binding_commitment: PublicKey,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Nonce {
    pub hiding_nonce: SecretKey,
    pub binding_nonce: SecretKey,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NonceShare {
    pub participant_id: ParticipantId,
    pub commitment: NonceCommitment,
    pub nonce: Option<Nonce>,
}

mod scalar_serde {
    use super::*;
    use serde::{Serializer, Deserializer};
    use serde::de::Error;

    pub fn serialize<S: Serializer>(scalar: &Scalar, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_bytes(&scalar.to_be_bytes())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Scalar, D::Error> {
        let bytes: Vec<u8> = Deserialize::deserialize(d)?;
        let arr: [u8; 32] = bytes.try_into().map_err(|_| D::Error::custom("invalid scalar length"))?;
        Scalar::from_be_bytes(arr).map_err(|_| D::Error::custom("invalid scalar value")) // ok_or_else?
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartialSignature {
    pub participant_id: ParticipantId,
    #[serde(with = "scalar_serde")]
    pub signature_share: Scalar,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SigningCommitments {
    pub commitments: HashMap<ParticipantId, NonceCommitment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SigningPackage {
    pub message: Vec<u8>,
    pub signing_commitments: SigningCommitments,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DkgRound1Package {
    pub participant_id: ParticipantId,
    pub commitments: Vec<PublicKey>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DkgRound2Package {
    pub participant_id: ParticipantId,
    pub secret_shares: HashMap<ParticipantId, SecretKey>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrostSignature {
    pub signature: secp256k1::schnorr::Signature,
    pub participants: Vec<ParticipantId>,
}

impl KeyShare {
    pub fn new(
        participant_id: ParticipantId,
        secret_share: SecretKey,
        public_key_share: PublicKey,
        threshold: u32,
        total_participants: u32,
    ) -> Self {
        Self {
            participant_id,
            secret_share,
            public_key_share,
            threshold,
            total_participants,
        }
    }
}

impl NonceShare {
    pub fn new(participant_id: ParticipantId, nonce: Nonce) -> Self {
        let secp = secp256k1::Secp256k1::new();
        let hiding_commitment = PublicKey::from_secret_key(&secp, &nonce.hiding_nonce);
        let binding_commitment = PublicKey::from_secret_key(&secp, &nonce.binding_nonce);

        Self {
            participant_id,
            commitment: NonceCommitment {
                hiding_commitment,
                binding_commitment,
            },
            nonce: Some(nonce),
        }
    }

    pub fn commitment_only(participant_id: ParticipantId, commitment: NonceCommitment) -> Self {
        Self {
            participant_id,
            commitment,
            nonce: None,
        }
    }
}