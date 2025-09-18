use std::{
    array::TryFromSliceError,
    collections::HashMap,
    fmt::{Display, Formatter},
};

use bitcoin::{
    hashes::{FromSliceError, sha256::Hash},
    secp256k1::{Error as Secp256k1Error, PublicKey},
};
use thiserror::Error;
use uuid::Uuid;
use spark_address::SparkAddressError;
use token_identifier::TokenIdentifier;

use crate::{
    spark_hash::SparkHash,
    spark_signature::SparkSignature,
    token_leaf::{TokenLeafOutput, TokenLeafToSpend},
};

/// Represents a version of a token transaction
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenTransactionVersion {
    /// Token transfers V1
    V1,

    /// Token transfers V2
    V2,

    /// Token transfers V3
    V3,

    /// Token transfers V4
    V4,
}

impl TokenTransactionVersion {
    /// Serializes token transaction version to bytes
    pub fn bytes(&self) -> [u8; 4] {
        match self {
            TokenTransactionVersion::V1 => 0u32.to_be_bytes(),
            TokenTransactionVersion::V2 => 1u32.to_be_bytes(),
            TokenTransactionVersion::V3 => 2u32.to_be_bytes(),
            TokenTransactionVersion::V4 => 3u32.to_be_bytes(),
        }
    }

    /// Converts the token transaction version to u32
    pub fn as_u32(&self) -> u32 {
        match self {
            TokenTransactionVersion::V1 => 0u32,
            TokenTransactionVersion::V2 => 1u32,
            TokenTransactionVersion::V3 => 2u32,
            TokenTransactionVersion::V4 => 3u32,
        }
    }
}

impl Display for TokenTransactionVersion {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            TokenTransactionVersion::V1 => write!(f, "V1"),
            TokenTransactionVersion::V2 => write!(f, "V2"),
            TokenTransactionVersion::V3 => write!(f, "V3"),
            TokenTransactionVersion::V4 => write!(f, "V4"),
        }
    }
}

/// Represents a token transaction following the LRC20 specification.
///
/// This struct contains the input and output information for a token transaction.
/// It includes the input type (Mint, Transfer, etc.), the leaves to create,
/// the operator identity public keys, and the network information.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TokenTransaction {
    /// The version (should always be V2)
    pub version: TokenTransactionVersion,

    /// The input type of the token transaction.
    pub input: TokenTransactionInput,

    /// The leaves to create in the token transaction.
    pub leaves_to_create: Vec<TokenLeafOutput>,

    /// The operator identity public keys for the token transaction.
    pub spark_operator_identity_public_keys: Vec<PublicKey>,

    /// Server-set expiry time.
    pub expiry_time: u64,

    /// The network information for the token transaction.
    pub network: Option<u32>,

    /// The timestamp of when the client created the transaction.
    pub client_created_timestamp: u64,

    /// The Spark Invoices
    pub invoice_attachments: HashMap<Uuid, String>,
}

impl TokenTransaction {
    /// Computes the hash of the token transaction.
    ///
    /// This method converts the token transaction into a hash using the `SparkHash` type.
    /// It returns the computed hash or an error if the conversion fails.
    ///
    /// # Returns
    /// A `SparkHash` representing the hash of the token transaction.
    pub fn hash(&self) -> Result<SparkHash, TokenTransactionError> {
        self.try_into()
            .map_err(|err| TokenTransactionError::HashError(err))
    }
}

impl TryFrom<&TokenTransaction> for Hash {
    type Error = TokenTransactionError;

    fn try_from(tx: &TokenTransaction) -> Result<Self, Self::Error> {
        let spark_hash = SparkHash::try_from(tx)?;
        Ok(*spark_hash)
    }
}

/// Represents the input type of a token transaction following the LRC20 specification.
///
/// This enum defines the different types of token transactions that can be created.
/// It includes minting, transferring, and other possible transaction types.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TokenTransactionInput {
    /// Represents an LRC20 mint transaction.
    Mint(TokenTransactionMintInput),

    /// Represents an LRC20 transfer transaction.
    Transfer(TokenTransactionTransferInput),

    /// Represents an LRC20 create(announce) transaction.
    Create(TokenTransactionCreateInput),
}

/// Represents an LRC20 mint transaction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TokenTransactionMintInput {
    /// The public key of the issuer of the token.
    pub issuer_public_key: PublicKey,

    /// The signature of the issuer of the token.
    pub issuer_signature: Option<OperatorSpecificSignatureData>,

    /// The timestamp of the mint transaction. (None for V1)
    pub issuer_provided_timestamp: Option<u64>,

    /// The token identifier. (None for V0)
    pub token_identifier: Option<TokenIdentifier>,
}

/// Represents an LRC20 transfer transaction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TokenTransactionTransferInput {
    /// The leaves to spend in the transfer transaction.
    pub leaves_to_spend: Vec<TokenLeafToSpend>,
}

/// Represents an LRC20 create transaction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TokenTransactionCreateInput {
    /// The issuer public key
    pub issuer_public_key: PublicKey,

    /// The token name
    pub token_name: String,

    /// The token ticker
    pub token_ticker: String,

    /// The decimals
    pub decimals: u32,

    /// The max supply
    pub max_supply: u128,

    /// The flag represents whether the token can be frozen
    pub is_freezable: bool,

    /// The creation entity public key
    pub creation_entity_public_key: Option<PublicKey>,
}

/// Represents the operator specific signature data for a token transaction.
///
/// This struct contains the identity public key and the signature of the operator.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct OperatorSpecificSignatureData {
    /// The public key of the operator.
    pub identity_public_key: Option<PublicKey>,

    /// The signature of the operator.
    pub signature: SparkSignature,
}

/// Errors that can occur when working with `TokenTransaction` operations.
#[derive(Error, Debug)]
pub enum TokenTransactionError {
    /// Hashing the token transaction failed.
    #[error("Failed to hash token transaction: {0}")]
    HashError(#[from] crate::spark_hash::SparkHashError),

    /// Invalid token transaction version.
    #[error("Invalid token transaction version: {0}")]
    InvalidTokenTransactionVersion(u32),

    /// Invalid token transaction input for this version.
    #[error("Invalid token transaction input: {0}")]
    InvalidTokenTransactionInput(String),

    /// Network is missing.
    #[error("Network is missing")]
    NetworkMissing,

    /// Token input is missing.
    #[error("Token input is missing")]
    TokenInputMissing,

    /// Token identifier is missing.
    #[error("Token identifier is missing")]
    TokenIdentifierMissing,

    /// Revocation public key is missing.
    #[error("Revocation public key is missing")]
    RevocationPublicKeyMissing,

    /// secp256k1 error while parsing/handling public keys
    #[error("Secp256k1 error: {0}")]
    Secp256k1(#[from] Secp256k1Error),

    /// Byte slice had an invalid length when converting
    #[error("Invalid slice length: {0}")]
    TryFromSlice(#[from] TryFromSliceError),

    /// Failed to parse hash bytes from slice
    #[error("Invalid hash bytes: {0}")]
    FromSlice(#[from] FromSliceError),

    /// Failed to parse token identifier
    #[error("Token identifier parse error: {0}")]
    TokenIdentifierParse(#[from] token_identifier::TokenIdentifierParseError),

    /// Failed to parse Spark Invoice
    #[error("Spark Invoice parse error: {0}")]
    InvoiceParse(#[from] SparkAddressError),

    /// Invoice data is missing.
    #[error("Invoice data is missing")]
    InvoiceDataMissing
}
