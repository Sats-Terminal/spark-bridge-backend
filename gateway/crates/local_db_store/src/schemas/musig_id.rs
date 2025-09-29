use bitcoin::secp256k1::PublicKey;
use serde::{Deserialize, Serialize};

pub type RuneId = String;

/// Struct represents inner logic that is stored in db to be as convenient as previous api
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum MusigId {
    User {
        user_public_key: PublicKey,
        rune_id: RuneId,
    },
    Issuer {
        issuer_public_key: PublicKey,
        rune_id: RuneId,
    },
}

impl MusigId {
    pub fn get_public_key(&self) -> PublicKey {
        match self {
            MusigId::User { user_public_key, .. } => *user_public_key,
            MusigId::Issuer { issuer_public_key, .. } => *issuer_public_key,
        }
    }

    pub fn get_rune_id(&self) -> String {
        match self {
            MusigId::User { rune_id, .. } => rune_id.clone(),
            MusigId::Issuer { rune_id, .. } => rune_id.clone(),
        }
    }

    pub fn is_issuer(&self) -> bool {
        match self {
            MusigId::User { .. } => false,
            MusigId::Issuer { .. } => true,
        }
    }
}
