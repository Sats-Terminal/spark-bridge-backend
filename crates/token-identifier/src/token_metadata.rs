use std::{
    fmt,
    io::{self, Cursor, Read},
};

use bitcoin::{
    Network,
    consensus::ReadExt,
    hashes::{Hash, HashEngine, sha256},
    key::constants::PUBLIC_KEY_SIZE,
    p2p::Magic,
    secp256k1::PublicKey,
};

use crate::TokenIdentifier;

/// L1 Creation Entity Public Key - used for L1 tokens
/// This corresponds to a 33-byte public key with all zeros
pub const L1_CREATION_ENTITY_PUBLIC_KEY: [u8; 33] = [0u8; 33];

#[cfg(test)]
use hex;

/// The maximum size of the name in [`TokenMetadata`] in bytes.
pub const MAX_NAME_SIZE: usize = 20;

/// The minimum size of the name in [`TokenMetadata`] in bytes.
pub const MIN_NAME_SIZE: usize = 3;

/// The maximum size of the symbol in [`TokenMetadata`] in bytes.
pub const MAX_SYMBOL_SIZE: usize = 6;

/// The minimum size of the symbol in [`TokenMetadata`] in bytes.
pub const MIN_SYMBOL_SIZE: usize = 3;

/// The minimum size of the token metadata in bytes.
pub const MIN_TOKEN_METADATA_SIZE: usize = PUBLIC_KEY_SIZE
    + size_of::<u8>()
    + MIN_NAME_SIZE
    + size_of::<u8>()
    + MIN_SYMBOL_SIZE
    + size_of::<u8>()
    + size_of::<u128>()
    + size_of::<bool>()
    + size_of::<u8>()
    + size_of::<Network>();

/// The maximum size of the token metadata in bytes.
pub const MAX_TOKEN_METADATA_SIZE: usize = PUBLIC_KEY_SIZE
    + size_of::<u8>()
    + MAX_NAME_SIZE
    + size_of::<u8>()
    + MAX_SYMBOL_SIZE
    + size_of::<u8>()
    + size_of::<u128>()
    + size_of::<bool>()
    + size_of::<u8>()
    + PUBLIC_KEY_SIZE
    + size_of::<Network>();

/// LRC20 token metadata.
#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TokenMetadata {
    /// The public key of the issuer.
    pub issuer_public_key: PublicKey,
    /// The name of the token. e.g. Bitcoin.
    pub name: String,
    /// The symbol (i.e. the short name) of the token. e.g. `BTC` for Bitcoin. The maximum size is
    /// [`MAX_SYMBOL_SIZE`], the minimum is [`MIN_SYMBOL_SIZE`].
    pub symbol: String,
    /// The number of decimal places for the token. e.g. 8 for Bitcoin.
    pub decimal: u8,
    /// The maximum supply of the token. e.g. 21_000_000 for Bitcoin.
    pub max_supply: u128,
    /// Indicates whether the token can be freezed or not by the issuer.
    pub is_freezable: bool,
    /// The public key of the entity that created the token.
    pub creation_entity_public_key: Option<PublicKey>,
    /// The network.
    pub network: Network,
}

impl TokenMetadata {
    /// Creates a new LRC20 token metadata.
    pub fn new(
        issuer_public_key: PublicKey,
        name: String,
        symbol: String,
        decimal: u8,
        max_supply: u128,
        is_freezable: bool,
        creation_entity_public_key: Option<PublicKey>,
        network: Network,
    ) -> Self {
        Self {
            issuer_public_key,
            name,
            symbol,
            decimal,
            max_supply,
            is_freezable,
            creation_entity_public_key,
            network,
        }
    }

    /// Computes the token identifier for the LRC20 token metadata.
    pub fn compute_token_identifier(&self) -> TokenIdentifier {
        let mut engine = sha256::Hash::engine();
        // Try concatenation approach - maybe Go concatenates all hashes then takes SHA256
        let mut all_hashes = Vec::new();

        // Hash version (1 byte)
        let version_hash = sha256::Hash::hash(&[1u8]);
        #[cfg(test)]
        println!(
            "Step 1 - Version: input=[01], hash={}",
            hex::encode(version_hash.as_byte_array())
        );
        all_hashes.extend_from_slice(version_hash.as_byte_array());
        engine.input(version_hash.as_byte_array());

        // Issuer public key (33 bytes)
        let issuer_key_bytes = self.issuer_public_key.serialize();
        let issuer_public_key_hash = sha256::Hash::hash(&issuer_key_bytes);
        #[cfg(test)]
        println!(
            "Step 2 - Issuer key: input={}, hash={}",
            hex::encode(&issuer_key_bytes),
            hex::encode(issuer_public_key_hash.as_byte_array())
        );
        all_hashes.extend_from_slice(issuer_public_key_hash.as_byte_array());
        engine.input(issuer_public_key_hash.as_byte_array());

        // Name (variable length)
        let name_bytes = self.name.as_bytes();
        let name_hash = sha256::Hash::hash(name_bytes);
        #[cfg(test)]
        println!(
            "Step 3 - Name: input={} ({}), hash={}",
            hex::encode(name_bytes),
            self.name,
            hex::encode(name_hash.as_byte_array())
        );
        all_hashes.extend_from_slice(name_hash.as_byte_array());
        engine.input(name_hash.as_byte_array());

        // Symbol (variable length)
        let symbol_bytes = self.symbol.as_bytes();
        let symbol_hash = sha256::Hash::hash(symbol_bytes);
        #[cfg(test)]
        println!(
            "Step 4 - Symbol: input={} ({}), hash={}",
            hex::encode(symbol_bytes),
            self.symbol,
            hex::encode(symbol_hash.as_byte_array())
        );
        all_hashes.extend_from_slice(symbol_hash.as_byte_array());
        engine.input(symbol_hash.as_byte_array());

        // Decimal (1 byte)
        let decimal_bytes = [self.decimal];
        let decimal_hash = sha256::Hash::hash(&decimal_bytes);
        #[cfg(test)]
        println!(
            "Step 5 - Decimal: input={}, hash={}",
            hex::encode(&decimal_bytes),
            hex::encode(decimal_hash.as_byte_array())
        );
        all_hashes.extend_from_slice(decimal_hash.as_byte_array());
        engine.input(decimal_hash.as_byte_array());

        // Max supply (16 bytes)
        let max_supply_bytes = self.max_supply.to_be_bytes();
        let max_supply_hash = sha256::Hash::hash(&max_supply_bytes);
        #[cfg(test)]
        println!(
            "Step 6 - Max supply: input={}, hash={}",
            hex::encode(&max_supply_bytes),
            hex::encode(max_supply_hash.as_byte_array())
        );
        all_hashes.extend_from_slice(max_supply_hash.as_byte_array());
        engine.input(max_supply_hash.as_byte_array());

        // Hash freezable flag (1 byte) - matches Go's trueHash/falseHash
        let (_freezable_input, is_freezable_hash) = if self.is_freezable {
            let input = [1u8];
            (input, sha256::Hash::hash(&input)) // trueHash equivalent
        } else {
            let input = [0u8];
            (input, sha256::Hash::hash(&input)) // falseHash equivalent
        };
        #[cfg(test)]
        println!(
            "Step 7 - Freezable: input={}, hash={}",
            hex::encode(&_freezable_input),
            hex::encode(is_freezable_hash.as_byte_array())
        );
        all_hashes.extend_from_slice(is_freezable_hash.as_byte_array());
        engine.input(is_freezable_hash.as_byte_array());

        // Hash network (4 bytes) - matches Go's binary.BigEndian.AppendUint32 (big-endian)
        let le_magic = self.network.magic().to_bytes(); // little-endian as in wire order
        let magic_u32 = u32::from_le_bytes(le_magic);
        let be_magic = magic_u32.to_be_bytes();
        let network_hash = sha256::Hash::hash(&be_magic);
        #[cfg(test)]
        println!(
            "Step 8 - Network: input={}, hash={}",
            hex::encode(&be_magic),
            hex::encode(network_hash.as_byte_array())
        );
        all_hashes.extend_from_slice(network_hash.as_byte_array());
        engine.input(network_hash.as_byte_array());

        // Token create layer logic - matches Go implementation exactly:
        let creation_entity_hash = if let Some(creation_entity_public_key) = &self.creation_entity_public_key {
            // Spark token: layer byte (1) + creation entity public key (33 bytes, compressed)
            let mut bytes = vec![2u8]; // TokenCreateLayerSpark (layer byte 2)
            bytes.extend_from_slice(&creation_entity_public_key.serialize());
            let hash = sha256::Hash::hash(&bytes);
            #[cfg(test)]
            println!(
                "Step 9 - Creation entity (Spark): input={}, hash={}",
                hex::encode(&bytes),
                hex::encode(hash.as_byte_array())
            );
            hash
        } else {
            // L1 token: just layer byte (0)
            let bytes = [1u8];
            let hash = sha256::Hash::hash(&bytes);
            #[cfg(test)]
            println!(
                "Step 9 - Creation entity (L1): input={}, hash={}",
                hex::encode(&bytes),
                hex::encode(hash.as_byte_array())
            );
            hash
        };
        all_hashes.extend_from_slice(creation_entity_hash.as_byte_array());
        engine.input(creation_entity_hash.as_byte_array());

        // Final hash (streaming)
        let final_hash = sha256::Hash::from_engine(engine);
        #[cfg(test)]
        {
            println!("Concatenated hashes length: {} bytes", all_hashes.len());
            println!("Concatenated hashes: {}", hex::encode(&all_hashes));
            println!("Final hash: {}", hex::encode(final_hash.as_byte_array()));
        }

        final_hash.into()
    }

    /// Validates the LRC20 token metadata.
    pub fn validate(&self) -> Result<(), TokenMetadataParseError> {
        if self.name.len() < MIN_NAME_SIZE || self.name.len() > MAX_NAME_SIZE {
            return Err(TokenMetadataParseError::InvalidNameLength(self.name.len()));
        }

        if self.symbol.len() < MIN_SYMBOL_SIZE || self.symbol.len() > MAX_SYMBOL_SIZE {
            return Err(TokenMetadataParseError::InvalidSymbolLength(self.symbol.len()));
        }

        Ok(())
    }

    /// Deserializes the LRC20 token metadata from bytes.
    pub fn from_bytes(data: &[u8]) -> Result<Self, TokenMetadataParseError> {
        if data.len() < MIN_TOKEN_METADATA_SIZE {
            return Err(TokenMetadataParseError::InvalidSize(data.len()));
        }

        let mut cursor = Cursor::new(data);

        // Read the issuer public key
        let mut issuer_public_key_bytes = [0u8; PUBLIC_KEY_SIZE];

        cursor
            .read(&mut issuer_public_key_bytes)
            .map_err(|err| wrap_io_error(err, "failed to read the token_identifier"))?;

        let issuer_public_key = PublicKey::from_slice(&issuer_public_key_bytes)
            .map_err(|err| wrap_io_error(err, "failed to read the issuer public key"))?;

        // Read the name
        let name_len = cursor
            .read_u8()
            .map_err(|err| wrap_io_error(err, "failed to read the name length"))? as usize;

        if !(MIN_NAME_SIZE..=MAX_NAME_SIZE).contains(&name_len) {
            return Err(TokenMetadataParseError::InvalidNameLength(name_len));
        }

        let mut name_bytes = vec![0; name_len];
        cursor
            .read_exact(&mut name_bytes)
            .map_err(|err| wrap_io_error(err, "failed to read the name"))?;

        let name = String::from_utf8(name_bytes).map_err(|err| wrap_io_error(err, "failed to read the name"))?;

        // Read the symbol
        let symbol_len = cursor
            .read_u8()
            .map_err(|err| wrap_io_error(err, "failed to read the symbol length"))? as usize;

        if !(MIN_SYMBOL_SIZE..=MAX_SYMBOL_SIZE).contains(&symbol_len) {
            return Err(TokenMetadataParseError::InvalidSymbolLength(symbol_len));
        }

        let mut symbol_bytes = vec![0; symbol_len];
        cursor
            .read_exact(&mut symbol_bytes)
            .map_err(|err| wrap_io_error(err, "failed to read the symbol"))?;

        let symbol = String::from_utf8(symbol_bytes).map_err(|err| wrap_io_error(err, "failed to read the symbol"))?;

        // Read the decimal
        let decimal = cursor
            .read_u8()
            .map_err(|err| wrap_io_error(err, "failed to read the decimal"))?;

        // Read the max_supply
        let mut max_supply_bytes = vec![0; size_of::<u128>()];
        cursor
            .read_exact(&mut max_supply_bytes)
            .map_err(|err| wrap_io_error(err, "failed to read the max supply"))?;
        let max_supply = u128::from_le_bytes(max_supply_bytes.try_into().unwrap());

        // Read the is_freezable
        let is_freezable = cursor
            .read_u8()
            .map_err(|err| wrap_io_error(err, "failed to read is freezable"))?;

        // Read the creation entity public key
        let creation_entity_public_key_flag = cursor
            .read_u8()
            .map_err(|err| wrap_io_error(err, "failed to read the creation entity public key flag"))?;

        let creation_entity_public_key = if creation_entity_public_key_flag != 0 {
            let mut creation_entity_public_key_bytes = [0u8; PUBLIC_KEY_SIZE]; // 33 bytes
            cursor
                .read_exact(&mut creation_entity_public_key_bytes)
                .map_err(|err| wrap_io_error(err, "failed to read the creation entity public key bytes"))?;
            Some(
                PublicKey::from_slice(&creation_entity_public_key_bytes)
                    .map_err(|err| wrap_io_error(err, "failed to parse the creation entity public key"))?,
            )
        } else {
            None
        };

        // Read the network
        let mut network_bytes = [0u8; 4];
        cursor
            .read_exact(&mut network_bytes)
            .map_err(|err| wrap_io_error(err, "failed to read the network"))?;

        let network = Network::from_magic(Magic::from_bytes(network_bytes)).ok_or_else(|| {
            TokenMetadataParseError::IoError(io::Error::new(io::ErrorKind::InvalidData, "Invalid network magic"))
        })?;

        Ok(Self {
            issuer_public_key,
            name,
            symbol,
            decimal,
            max_supply,
            is_freezable: is_freezable != 0,
            creation_entity_public_key,
            network,
        })
    }

    /// Serializes the LRC20 token metadata to bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut result = Vec::with_capacity(MAX_TOKEN_METADATA_SIZE);

        result.extend_from_slice(&self.issuer_public_key.serialize());
        result.push(self.name.len() as u8);
        result.extend_from_slice(self.name.as_bytes());
        result.push(self.symbol.len() as u8);
        result.extend_from_slice(self.symbol.as_bytes());
        result.push(self.decimal);
        result.extend_from_slice(&self.max_supply.to_le_bytes());
        result.push(if self.is_freezable { 1 } else { 0 });

        if let Some(creation_entity_public_key) = &self.creation_entity_public_key {
            result.push(1);
            result.extend_from_slice(&creation_entity_public_key.serialize());
        } else {
            result.push(0);
        }

        result.extend_from_slice(&self.network.magic().to_bytes());

        result
    }
}

impl From<TokenMetadata> for TokenIdentifier {
    fn from(metadata: TokenMetadata) -> Self {
        metadata.compute_token_identifier()
    }
}

impl From<&TokenMetadata> for TokenIdentifier {
    fn from(metadata: &TokenMetadata) -> Self {
        metadata.compute_token_identifier()
    }
}

fn wrap_io_error(err: impl fmt::Display, message: &str) -> TokenMetadataParseError {
    TokenMetadataParseError::IoError(io::Error::other(format!("{}: {}", message, err)))
}

/// Error type for parsing LRC20 token metadata.
#[derive(Debug)]
pub enum TokenMetadataParseError {
    /// The name length is invalid.
    InvalidNameLength(usize),
    /// The symbol length is invalid.
    InvalidSymbolLength(usize),
    /// The size is invalid.
    InvalidSize(usize),
    /// An I/O error occurred.
    IoError(io::Error),
}

impl fmt::Display for TokenMetadataParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TokenMetadataParseError::InvalidNameLength(length) => {
                write!(f, "Invalid name length: {}", length)
            }
            TokenMetadataParseError::InvalidSymbolLength(length) => {
                write!(f, "Invalid symbol length: {}", length)
            }
            TokenMetadataParseError::InvalidSize(size) => {
                write!(f, "Invalid size: {}", size)
            }
            TokenMetadataParseError::IoError(e) => write!(f, "Parse error: {}", e),
        }
    }
}

#[cfg(not(feature = "no-std"))]
impl std::error::Error for TokenMetadataParseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            TokenMetadataParseError::InvalidNameLength(_) => None,
            TokenMetadataParseError::InvalidSymbolLength(_) => None,
            TokenMetadataParseError::InvalidSize(_) => None,
            TokenMetadataParseError::IoError(e) => Some(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use core::str::FromStr;
    use std::io;

    use bitcoin::{Network, secp256k1::PublicKey};
    use hex;
    use once_cell::sync::Lazy;

    use crate::token_metadata::{MIN_TOKEN_METADATA_SIZE, TokenMetadata, TokenMetadataParseError};

    static TEST_ISSUER_PUBLIC_KEY: Lazy<PublicKey> = Lazy::new(|| {
        PublicKey::from_str("0305bd561c55adf4a2369f936ab82f7688438f1b199af2455c5ef33935e64ee7e7").unwrap()
    });

    static TEST_METADATA: Lazy<TokenMetadata> = Lazy::new(|| {
        TokenMetadata::new(
            *TEST_ISSUER_PUBLIC_KEY,
            "Test Token".to_string(),
            "TEST".to_string(),
            8,
            0,
            true,
            Some(*TEST_ISSUER_PUBLIC_KEY),
            Network::Regtest,
        )
    });

    #[test]
    fn test_compute_token_identifier() {
        let token_identifier = TEST_METADATA.compute_token_identifier();

        // Note: This hash has changed due to algorithm updates to match Go implementation
        assert_eq!(
            token_identifier.to_string(),
            "e468b91d8059c0363650e63ef32443f29ccf49013f5bac6052fa6cac45a61317"
        );
    }

    /// Test that matches the production L1 token from Go tests
    #[test]
    fn test_actual_production_l1_token_identifier() {
        use hex;

        let issuer_public_key =
            PublicKey::from_str("036898ed2b633947f0994b8952fa06da2cfc7d1ee003fcf2cc076752b9ad3b3691").unwrap();

        let max_supply_bytes = hex::decode("00000000000000000000000000009c3f").unwrap();
        let max_supply = u128::from_be_bytes({
            let mut bytes = [0u8; 16];
            bytes.copy_from_slice(&max_supply_bytes);
            bytes
        });

        // This is an actual token created in production servers on Regtest
        let metadata = TokenMetadata {
            issuer_public_key,
            name: "RaccoonCoin".to_string(),
            symbol: "RCC".to_string(),
            decimal: 10,
            max_supply,
            is_freezable: false,
            creation_entity_public_key: None, // L1 token uses None
            network: Network::Regtest,
        };

        let token_identifier = metadata.compute_token_identifier();

        // IMPORTANT: This expected value should match Go implementation!
        let expected_bytes = hex::decode("f1ca1e65691d0f65132ce24608594aaccd741e323056c97407a9f625b0ee4251").unwrap();

        assert_eq!(token_identifier.to_bytes().to_vec(), expected_bytes);
    }

    /// Test that matches the production Spark token from Go tests
    #[test]
    fn test_actual_production_spark_token_identifier() {
        use hex;

        let issuer_public_key =
            PublicKey::from_str("036898ed2b633947f0994b8952fa06da2cfc7d1ee003fcf2cc076752b9ad3b3691").unwrap();

        // Use the same compressed key as in Go test
        let creation_entity_compressed_bytes =
            hex::decode("0345b806679a5e63159584db91fec038cffd2ef59cee031abe92e2f30bf0642175").unwrap();
        let creation_entity_compressed_key = PublicKey::from_slice(&creation_entity_compressed_bytes).unwrap();
        let creation_entity_public_key = creation_entity_compressed_key;

        let max_supply_bytes = hex::decode("00000000000000000000000000009c3f").unwrap();
        let max_supply = u128::from_be_bytes({
            let mut bytes = [0u8; 16];
            bytes.copy_from_slice(&max_supply_bytes);
            bytes
        });

        // This is an actual token created in production servers on Regtest
        let metadata = TokenMetadata {
            issuer_public_key,
            name: "RaccoonCoin".to_string(),
            symbol: "RCC".to_string(),
            decimal: 10,
            max_supply,
            is_freezable: false,
            creation_entity_public_key: Some(creation_entity_public_key),
            network: Network::Regtest,
        };

        let token_identifier = metadata.compute_token_identifier();

        // IMPORTANT: This expected value should match Go implementation!
        let expected_bytes = hex::decode("8b5fde73c803f6ef5c819ae94ddd035f02bee63555a08fc94f6851e289b46a1b").unwrap();

        assert_eq!(token_identifier.to_bytes().to_vec(), expected_bytes);
    }

    #[test]
    fn test_serialize_deserialize() {
        struct TestData {
            metadata: TokenMetadata,
            expect_error: bool,
        }

        let test_vector = vec![
            TestData {
                metadata: TokenMetadata {
                    issuer_public_key: *TEST_ISSUER_PUBLIC_KEY,
                    name: "TokenName".to_string(),
                    symbol: "TNK".to_string(),
                    decimal: 2,
                    max_supply: 1_000_000,
                    is_freezable: true,
                    creation_entity_public_key: Some(*TEST_ISSUER_PUBLIC_KEY),
                    network: Network::Regtest,
                },
                expect_error: false,
            },
            TestData {
                metadata: TokenMetadata {
                    issuer_public_key: *TEST_ISSUER_PUBLIC_KEY,
                    name: "TokenName17Charss".to_string(),
                    symbol: "TESTSY".to_string(),
                    decimal: 255,
                    max_supply: 18_446_744_073_709_551_615,
                    is_freezable: true,
                    creation_entity_public_key: None,
                    network: Network::Regtest,
                },
                expect_error: false,
            },
            TestData {
                metadata: TokenMetadata {
                    issuer_public_key: *TEST_ISSUER_PUBLIC_KEY,
                    name: "TokenName".to_string(),
                    symbol: "TNK".to_string(),
                    decimal: 2,
                    max_supply: 1_000_000,
                    is_freezable: false,
                    creation_entity_public_key: None,
                    network: Network::Regtest,
                },
                expect_error: false,
            },
            TestData {
                metadata: TokenMetadata {
                    issuer_public_key: *TEST_ISSUER_PUBLIC_KEY,
                    name: "The String Longer Than MAX_NAME_SIZE".to_string(),
                    symbol: "TNK".to_string(),
                    decimal: 2,
                    max_supply: 1_000_000,
                    is_freezable: true,
                    creation_entity_public_key: None,
                    network: Network::Regtest,
                },
                expect_error: true,
            },
            TestData {
                metadata: TokenMetadata {
                    issuer_public_key: *TEST_ISSUER_PUBLIC_KEY,
                    name: "TokenName".to_string(),
                    symbol: "The String Longer Than MAX_SYMBOL_SIZE".to_string(),
                    decimal: 2,
                    max_supply: 1_000_000,
                    is_freezable: true,
                    creation_entity_public_key: None,
                    network: Network::Regtest,
                },
                expect_error: true,
            },
            TestData {
                metadata: TokenMetadata {
                    issuer_public_key: *TEST_ISSUER_PUBLIC_KEY,
                    name: "".to_string(),
                    symbol: "TNK".to_string(),
                    decimal: 2,
                    max_supply: 1_000_000,
                    is_freezable: true,
                    creation_entity_public_key: None,
                    network: Network::Regtest,
                },
                expect_error: true,
            },
            TestData {
                metadata: TokenMetadata {
                    issuer_public_key: *TEST_ISSUER_PUBLIC_KEY,
                    name: "TokenName".to_string(),
                    symbol: "".to_string(),
                    decimal: 2,
                    max_supply: 1_000_000,
                    is_freezable: true,
                    creation_entity_public_key: None,
                    network: Network::Regtest,
                },
                expect_error: true,
            },
        ];

        for test in test_vector {
            let data = test.metadata.to_bytes();
            println!("data: {:?}", data);
            match TokenMetadata::from_bytes(&data) {
                Ok(metadata) => {
                    assert_eq!(metadata, test.metadata);
                }
                Err(err) => {
                    assert!(test.expect_error, "Unexpected error: {}", err);
                }
            }
        }
    }

    #[test]
    fn parse_invalid_bytes() {
        struct TestData {
            bytes: Vec<u8>,
            err: TokenMetadataParseError,
        }

        let test_vector = vec![
            TestData {
                bytes: vec![0],
                err: TokenMetadataParseError::InvalidSize(1),
            },
            TestData {
                bytes: vec![0; MIN_TOKEN_METADATA_SIZE],
                err: TokenMetadataParseError::IoError(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "failed to read the issuer public key: malformed public key",
                )),
            },
        ];

        for test in test_vector {
            let Err(err) = TokenMetadata::from_bytes(&test.bytes) else {
                panic!("Expected error: {:?}", test.err);
            };

            assert_eq!(err.to_string(), test.err.to_string());
        }
    }

    #[test]
    fn test_backward_compatibility() {
        let valid_metadata = vec![
            vec![
                3, 5, 189, 86, 28, 85, 173, 244, 162, 54, 159, 147, 106, 184, 47, 118, 136, 67, 143, 27, 25, 154, 242,
                69, 92, 94, 243, 57, 53, 230, 78, 231, 231, 17, 84, 111, 107, 101, 110, 78, 97, 109, 101, 49, 55, 67,
                104, 97, 114, 115, 115, 6, 84, 69, 83, 84, 83, 89, 255, 255, 255, 255, 255, 255, 255, 255, 255, 0, 0,
                0, 0, 0, 0, 0, 0, 1, 0, 250, 191, 181, 218,
            ],
            vec![
                3, 5, 189, 86, 28, 85, 173, 244, 162, 54, 159, 147, 106, 184, 47, 118, 136, 67, 143, 27, 25, 154, 242,
                69, 92, 94, 243, 57, 53, 230, 78, 231, 231, 9, 84, 111, 107, 101, 110, 78, 97, 109, 101, 3, 84, 78, 75,
                2, 64, 66, 15, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 250, 191, 181, 218,
            ],
        ];

        for metadata in valid_metadata {
            assert!(TokenMetadata::from_bytes(&metadata).is_ok());
        }
    }
}
