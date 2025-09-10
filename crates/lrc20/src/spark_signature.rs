use bitcoin::secp256k1::{self, ecdsa, schnorr};
use serde::{Deserialize, Serialize};

/// Represents the signature of an operator for a token transaction.
///
/// This enum defines the different types of signatures that can be used for a token transaction.
/// It includes ECDSA and Schnorr signatures.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SparkSignature {
    /// Represents an ECDSA signature.
    ECDSA(ecdsa::Signature),

    /// Represents a Schnorr signature.
    Schnorr(schnorr::Signature),
}

impl From<ecdsa::Signature> for SparkSignature {
    fn from(sig: ecdsa::Signature) -> Self {
        Self::ECDSA(sig)
    }
}

impl From<schnorr::Signature> for SparkSignature {
    fn from(sig: schnorr::Signature) -> Self {
        Self::Schnorr(sig)
    }
}

impl SparkSignature {
    /// Try to deserialize bytes to either ECDSA or Schnorr signature. None is returned if failed
    /// to deserialize.
    pub fn try_from_slice(bytes: &[u8]) -> Result<Self, secp256k1::Error> {
        if let Ok(ecdsa_sig) = ecdsa::Signature::from_compact(bytes) {
            return Ok(ecdsa_sig.into());
        }

        if let Ok(schnorr_sig) = schnorr::Signature::from_slice(bytes) {
            return Ok(schnorr_sig.into());
        }

        if let Ok(ecdsa_sig) = ecdsa::Signature::from_compact(bytes) {
            return Ok(ecdsa_sig.into());
        }

        Err(secp256k1::Error::InvalidSignature)
    }

    /// Converts the signature to a byte array.
    ///
    /// This method serializes the signature into a byte array.
    /// It supports both DER and compact serialization formats.
    ///
    /// # Returns
    /// A byte array in `[u8; 64]` representing the signature.
    pub fn bytes(&self) -> [u8; 64] {
        match self {
            // TODO: probably should use DER encoding
            SparkSignature::ECDSA(signature) => signature.serialize_compact(),
            SparkSignature::Schnorr(signature) => signature.serialize(),
        }
    }
}
