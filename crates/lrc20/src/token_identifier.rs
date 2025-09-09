use std::{fmt, fmt::Display, ops::Deref, str::FromStr};

use bech32::{Bech32m, Hrp};
use bitcoin::{
    Network, WitnessVersion,
    hashes::{FromSliceError, Hash, sha256},
    hex::HexToArrayError,
};
use thiserror::Error;

/// Size of serialized [`sha256::Hash`] under the hood.
pub const TOKEN_IDENTIFIER_SIZE: usize = sha256::Hash::LEN;

/// The network type for a token identifier (matches protobuf Network enum).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[repr(i32)]
pub enum TokenNetwork {
    /// Unspecified network
    #[default]
    Unspecified = 0,
    /// Mainnet
    Mainnet = 1,
    /// Regtest
    Regtest = 2,
    /// Testnet
    Testnet = 3,
    /// Signet
    Signet = 4,
}

impl TokenNetwork {
    /// Returns the prefix for the token network.
    #[must_use]
    pub const fn prefix(&self) -> &'static str {
        match self {
            Self::Unspecified => "btkn", // Default to mainnet for unspecified
            Self::Mainnet => "btkn",
            Self::Testnet => "btknt",
            Self::Regtest => "btknrt",
            Self::Signet => "btkns",
        }
    }

    /// Parses a network from a prefix string.
    pub fn from_prefix(prefix: &str) -> Option<Self> {
        match prefix {
            "btkn" => Some(Self::Mainnet),
            "btknt" => Some(Self::Testnet),
            "btknrt" => Some(Self::Regtest),
            "btkns" => Some(Self::Signet),
            _ => None,
        }
    }

    /// Converts from protobuf network value.
    pub fn from_bitcoin_network(value: Network) -> Self {
        match value {
            Network::Bitcoin => Self::Mainnet,
            Network::Testnet => Self::Testnet,
            Network::Testnet4 => Self::Testnet,
            Network::Signet => Self::Signet,
            Network::Regtest => Self::Regtest,
        }
    }
}

/// Represensts the asset type of the LRC20 token and is defined by a hash of the token metadata.
#[derive(Clone, PartialEq, Eq, Hash, Debug, PartialOrd, Ord, Copy, serde::Serialize, serde::Deserialize)]
pub struct TokenIdentifier(sha256::Hash);

impl Deref for TokenIdentifier {
    type Target = sha256::Hash;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Display for TokenIdentifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TokenIdentifier {
    /// Creates a new TokenIdentifier from a Sha256 hash.
    pub fn new(hash: sha256::Hash) -> Self {
        Self(hash)
    }

    /// Returns a reference to the inner Sha256 hash.
    pub fn inner(&self) -> &sha256::Hash {
        &self.0
    }

    /// Returns a mutable reference to the inner Sha256 hash.
    pub fn inner_mut(&mut self) -> &mut sha256::Hash {
        &mut self.0
    }

    /// Consumes the TokenIdentifier and returns the owned Sha256 hash.
    pub fn inner_owned(self) -> sha256::Hash {
        self.0
    }

    /// Returns the byte array representation of the token identifier.
    pub fn to_bytes(&self) -> [u8; TOKEN_IDENTIFIER_SIZE] {
        self.0.to_byte_array()
    }

    /// Constructs a TokenIdentifier from a byte slice
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, TokenIdentifierParseError> {
        if bytes.len() < TOKEN_IDENTIFIER_SIZE {
            return Err(TokenIdentifierParseError::InvalidSize(bytes.len()));
        }

        Ok(Self(sha256::Hash::from_slice(bytes)?))
    }

    /// Returns an empty TokenIdentifier initialized with zero bytes.
    pub fn empty() -> Self {
        Self::new(sha256::Hash::from_byte_array([0u8; TOKEN_IDENTIFIER_SIZE]))
    }

    /// Encodes the token identifier into a human readable bech32m string.
    pub fn encode_bech32m(&self, network: Network) -> Result<String, TokenIdentifierParseError> {
        // Serialize protobuf to bytes
        let data_bytes = self.to_bytes();

        let token_network = TokenNetwork::from_bitcoin_network(network);
        let hrp_str = token_network.prefix();
        let hrp = Hrp::parse(hrp_str).map_err(|_| TokenIdentifierParseError::InvalidAddressType)?;

        let encoded =
            bech32::encode::<Bech32m>(hrp, &data_bytes).map_err(|_| TokenIdentifierParseError::InvalidAddressType)?;

        Ok(encoded)
    }

    /// Decodes a bech32m string into a token identifier.
    pub fn decode_bech32m(address: &str, network: Network) -> Result<Self, TokenIdentifierParseError> {
        let (hrp, data_bytes) = bech32::decode(address).map_err(|_| TokenIdentifierParseError::InvalidAddressType)?;

        // Validate that it's a known token network prefix
        let expected_token_network = TokenNetwork::from_bitcoin_network(network);
        let decoded_network =
            TokenNetwork::from_prefix(hrp.as_str()).ok_or(TokenIdentifierParseError::InvalidAddressType)?;

        if decoded_network != expected_token_network {
            return Err(TokenIdentifierParseError::InvalidAddressType);
        }

        // Validate data length
        if data_bytes.len() != TOKEN_IDENTIFIER_SIZE {
            return Err(TokenIdentifierParseError::InvalidSize(data_bytes.len()));
        }

        // Decode protobuf message
        let spark_token_id =
            TokenIdentifier::from_bytes(&data_bytes[..]).map_err(|_| TokenIdentifierParseError::InvalidAddressType)?;

        Ok(spark_token_id)
    }

    /// Parses a token identifier from a string
    pub fn from_str(s: &str, network: Network) -> Result<Self, TokenIdentifierParseError> {
        // Try bech32m first (human readable format)
        if let Ok(identifier) = Self::decode_bech32m(s, network) {
            return Ok(identifier);
        }

        // Fall back to hex format (raw hash)
        let identifier = sha256::Hash::from_str(s)?;
        Ok(Self::new(identifier))
    }
}

impl From<sha256::Hash> for TokenIdentifier {
    fn from(hash: sha256::Hash) -> Self {
        Self(hash)
    }
}

impl From<&sha256::Hash> for TokenIdentifier {
    fn from(hash: &sha256::Hash) -> Self {
        Self(*hash)
    }
}

impl From<&TokenIdentifier> for sha256::Hash {
    fn from(token_identifier: &TokenIdentifier) -> Self {
        token_identifier.0
    }
}

impl From<TokenIdentifier> for sha256::Hash {
    fn from(token_identifier: TokenIdentifier) -> Self {
        token_identifier.0
    }
}

/// Error type for parsing LRC20 token identifier.
#[derive(Debug, Error)]
pub enum TokenIdentifierParseError {
    /// The size of the hash is invalid.
    #[error("Invalid size: {0}")]
    InvalidSize(usize),

    /// The hash string is invalid.
    #[error("Invalid hash string: {0}")]
    InvalidHashString(HexToArrayError),

    /// The hash bytes are invalid.
    #[error("Invalid hash bytes: {0}")]
    InvalidHashBytes(FromSliceError),

    /// The address type is invalid.
    #[error("Invalid address type")]
    InvalidAddressType,

    /// The witness program version is invalid.
    #[error("Invalid witness program version: {0}")]
    InvalidWitnessProgramVersion(WitnessVersion),

    /// The witness program length is invalid.
    #[error("Invalid witness program length: {0}")]
    InvalidWitnessProgramLength(usize),
}

impl From<HexToArrayError> for TokenIdentifierParseError {
    fn from(err: HexToArrayError) -> Self {
        TokenIdentifierParseError::InvalidHashString(err)
    }
}

impl From<FromSliceError> for TokenIdentifierParseError {
    fn from(err: FromSliceError) -> Self {
        TokenIdentifierParseError::InvalidHashBytes(err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_documentation_example_compatibility() {
        // The documentation example appears to be a Bitcoin address format rather than
        // a valid token identifier encoded with our protobuf specification
        let doc_example = "btkn1qw508d6qejxtdg4y5r3zarvary0c5xw7kygt080";

        // Confirm this example doesn't parse as a valid token identifier
        assert!(TokenIdentifier::decode_bech32m(doc_example, Network::Bitcoin).is_err());
    }

    #[test]
    fn test_token_identifier_to_string() {
        let token_identifier_str = "btknrt19g4z52329g4z52329g4z52329g4z52329g4z52329g4z52329g4qvstsg6";
        let token_identifier = TokenIdentifier::decode_bech32m(token_identifier_str, Network::Regtest).unwrap();

        let encoded_identifier_str = token_identifier.encode_bech32m(Network::Regtest).unwrap();

        assert_eq!(encoded_identifier_str, token_identifier_str);
    }
}
