//! Minimal Spark address codec.
//! Avoids unsafe code and works in `no_std` + `alloc` environments.
//!
//! # Crate Overview
//! The **spark-address** crate encodes & decodes *Spark* Bech32m addresses. A Spark
//! address couples a compressed secp256k1 public key with a network identifier
//! (see [`Network`]) and represents them as human-friendly Bech32m strings like
//! `sp1…` or `sprt1…`.
//!
//! ```rust
//! use spark_address::{Network, SparkAddressData, decode_spark_address, encode_spark_address};
//!
//! let pubkey = "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
//! let data = SparkAddressData {
//!     identity_public_key: pubkey.into(),
//!     network: Network::Mainnet,
//! };
//! let addr = encode_spark_address(&data)?;
//! let decoded = decode_spark_address(&addr)?;
//! assert_eq!(decoded, data);
//! # Ok::<(), spark_address::SparkAddressError>(())
//! ```
//!
//! ## Feature Flags
//! * **`std`** *(default)* — Use the Rust standard library. Disable to build for
//!   `#![no_std]` + `alloc` targets.
//! * **`validate-secp256k1`** — Validate the public key using the `secp256k1` crate.
//!
//! ## MSRV
//! Minimum supported Rust version: **1.70**.

#![forbid(unsafe_code)]
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;
extern crate core;

use alloc::string::ToString;
// `Vec` / `String` come from `alloc` when `std` is disabled.
#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};
use core::fmt;
#[cfg(feature = "std")]
use std::{string::String, vec::Vec};

use bech32::{self, Bech32m, Hrp};
use hex::{decode as hex_to_bytes, encode as bytes_to_hex};
use spark_invoice::SparkInvoiceFields;
use spark_invoice::proto::{read_varint_u32, write_len_prefixed_bytes, write_varint_u32};
/* ------------------------------------------------------------- *
 *  Network ⇄ HRP                                                 *
 * ------------------------------------------------------------- */

/// Networks supported by Spark.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Network {
    /// Main Bitcoin network (`sp` prefix).
    Mainnet,
    /// Bitcoin testnet (`spt` prefix).
    Testnet,
    /// Signet (`sps` prefix).
    Signet,
    /// Regression-test network (`sprt` prefix).
    Regtest,
    /// Local development network (`spl` prefix).
    Local,
}

impl Network {
    fn hrp(self) -> &'static str {
        match self {
            Network::Mainnet => "sp",
            Network::Testnet => "spt",
            Network::Signet => "sps",
            Network::Regtest => "sprt",
            Network::Local => "spl",
        }
    }

    fn from_hrp(hrp: &str) -> Option<Self> {
        match hrp {
            "sp" => Some(Network::Mainnet),
            "spt" => Some(Network::Testnet),
            "sps" => Some(Network::Signet),
            "sprt" => Some(Network::Regtest),
            "spl" => Some(Network::Local),
            _ => None,
        }
    }
}

/* ------------------------------------------------------------- *
 *  Error type                                                    *
 * ------------------------------------------------------------- */

#[derive(Debug)]
pub enum SparkAddressError {
    /// The Bech32 string failed to decode.
    InvalidBech32(bech32::DecodeError),
    /// The human-readable part (HRP) does not correspond to a known [`Network`].
    UnknownPrefix(String),
    /// The checksum was valid **Bech32** but not **Bech32m**.
    InvalidVariant,
    /// The string mixes upper- and lower-case characters.
    MixedCase,
    /// The address exceeded the 90-character limit specified by BIP-350.
    InvalidLength,
    /// The embedded pseudo-protobuf payload was malformed.
    BadProto,
    /// Public key hex failed to decode.
    Hex(hex::FromHexError),
    /// Public key length differed from 33 bytes.
    WrongKeyLength(usize),
    #[cfg(feature = "validate-secp256k1")]
    /// The provided public key is not a valid compressed secp256k1 key.
    InvalidSecp256k1,
    /// Failure while encoding back into Bech32m.
    Bech32Encode(bech32::EncodeError),
}

impl fmt::Display for SparkAddressError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SparkAddressError::InvalidBech32(e) => write!(f, "bech32 decode error: {e}"),
            SparkAddressError::UnknownPrefix(p) => write!(f, "unknown HRP prefix: {p}"),
            SparkAddressError::InvalidVariant => write!(f, "bech32 variant is not Bech32m"),
            SparkAddressError::MixedCase => write!(f, "address contains mixed upper/lower case"),
            SparkAddressError::InvalidLength => write!(f, "address exceeds maximum length (90)"),
            SparkAddressError::BadProto => write!(f, "invalid proto payload"),
            SparkAddressError::Hex(e) => write!(f, "hex decode error: {e}"),
            SparkAddressError::WrongKeyLength(n) => {
                write!(f, "wrong pubkey length: {n} (expected 33)")
            }
            #[cfg(feature = "validate-secp256k1")]
            SparkAddressError::InvalidSecp256k1 => write!(f, "invalid secp256k1 pubkey"),
            SparkAddressError::Bech32Encode(e) => write!(f, "bech32 encode error: {e}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for SparkAddressError {}

impl From<bech32::DecodeError> for SparkAddressError {
    fn from(e: bech32::DecodeError) -> Self {
        Self::InvalidBech32(e)
    }
}

impl From<bech32::EncodeError> for SparkAddressError {
    fn from(e: bech32::EncodeError) -> Self {
        Self::Bech32Encode(e)
    }
}

impl From<hex::FromHexError> for SparkAddressError {
    fn from(e: hex::FromHexError) -> Self {
        Self::Hex(e)
    }
}

/* ------------------------------------------------------------- *
 *  SparkAddressData                                              *
 * ------------------------------------------------------------- */

/// Result of a successful decode, or input to `encode`.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SparkAddressData {
    /// Compressed secp256k1 public key, hex-encoded (`02/03 + 32 bytes`).
    pub identity_public_key: String,
    /// Network for which the address is intended (determines HRP prefix).
    pub network: Network,
    /// Optional Spark invoice data
    pub invoice: Option<SparkInvoiceFields>,
    /// Optional signature bytes
    pub signature: Option<Vec<u8>>,
}

/* ------------------------------------------------------------- *
 *  Tiny "proto" wrapper (field-1, wire-type 2)                   *
 * ------------------------------------------------------------- */

const PUBLIC_KEY_TAG: u8 = 0x0a; // (1 << 3) | 2
const INVOICE_TAG: u8 = 0x12;
const SIGNATURE_TAG: u8 = 0x1a;

fn encode_proto(key: &[u8], spark_invoice_fields: Option<SparkInvoiceFields>, signature: Option<Vec<u8>>) -> Vec<u8> {
    let mut out = Vec::with_capacity(2 + key.len());
    out.push(PUBLIC_KEY_TAG);
    // Compressed pubkeys are 33 bytes; fall back to error if ever larger.
    let key_len: u8 = key.len().try_into().expect("key length exceeds 255 bytes");
    out.push(key_len);
    out.extend_from_slice(key);

    if let Some(invoice_fields) = spark_invoice_fields {
        out.push(INVOICE_TAG);

        let invoice_bytes = invoice_fields.encode_proto();
        write_varint_u32(&mut out, invoice_bytes.len() as u32);
        out.extend_from_slice(&invoice_bytes);
    }

    if let Some(signature) = signature {
        out.push(SIGNATURE_TAG);
        write_len_prefixed_bytes(&mut out, &signature);
    }

    out
}

pub struct DecodedProto {
    pub key: Vec<u8>,
    pub invoice_fields: Option<SparkInvoiceFields>,
    pub signature: Option<Vec<u8>>,
}

fn decode_proto(buf: &[u8]) -> Result<DecodedProto, SparkAddressError> {
    let mut _pos = 0;

    // --- Public key ---
    if buf.is_empty() || buf[_pos] != PUBLIC_KEY_TAG {
        return Err(SparkAddressError::BadProto);
    }
    _pos += 1;

    if _pos >= buf.len() {
        return Err(SparkAddressError::BadProto);
    }
    let key_len = buf[_pos] as usize;
    _pos += 1;

    if _pos + key_len > buf.len() {
        return Err(SparkAddressError::BadProto);
    }
    let key = &buf[_pos.._pos + key_len];
    _pos += key_len;

    // --- Optional invoice fields ---
    let mut invoice_fields = None;
    if _pos < buf.len() && buf[_pos] == INVOICE_TAG {
        _pos += 1;

        let len = read_varint_u32(buf, &mut _pos).map_err(|_| SparkAddressError::BadProto)? as usize;
        if _pos + len > buf.len() {
            return Err(SparkAddressError::BadProto);
        }
        let (decoded, used) =
            SparkInvoiceFields::decode_proto(&buf[_pos.._pos + len]).map_err(|_| SparkAddressError::BadProto)?;
        invoice_fields = Some(decoded);
        _pos += used;
    }

    // --- Optional signature ---
    let mut signature = None;
    if _pos < buf.len() && buf[_pos] == SIGNATURE_TAG {
        _pos += 1;

        let len = read_varint_u32(buf, &mut _pos).map_err(|_| SparkAddressError::BadProto)? as usize;
        if _pos + len > buf.len() {
            return Err(SparkAddressError::BadProto);
        }
        signature = Some(buf[_pos.._pos + len].to_vec());
        _pos += len;
    }

    Ok(DecodedProto {
        key: key.to_vec(),
        invoice_fields,
        signature,
    })
}

/* ------------------------------------------------------------- *
 *  Public API                                                    *
 * ------------------------------------------------------------- */

/// Encode a `(pubkey, network)` into a Spark Bech32m address.
///
/// # Panics
///
/// This function will panic if the HRP (Human Readable Part) is invalid. This should never happen
/// in practice as the HRP is statically defined in the `Network` enum.
///
/// # Errors
///
/// This function will return an error if:
/// * The public key is invalid hex (`SparkAddressError::Hex`)
/// * The public key length is not 33 bytes (`SparkAddressError::WrongKeyLength`)
/// * The public key is invalid secp256k1 (when `validate-secp256k1` feature is enabled) (`SparkAddressError::InvalidSecp256k1`)
/// * The bech32 encoding fails (`SparkAddressError::Bech32Encode`)
pub fn encode_spark_address(data: SparkAddressData) -> Result<String, SparkAddressError> {
    #[cfg(feature = "validate-secp256k1")]
    validate_pubkey(&data.identity_public_key)?;

    let key_bytes = hex_to_bytes(&data.identity_public_key)?;
    if key_bytes.len() != 33 {
        return Err(SparkAddressError::WrongKeyLength(key_bytes.len()));
    }

    let proto = encode_proto(&key_bytes, data.invoice, data.signature);

    let hrp = Hrp::parse(data.network.hrp()).expect("static HRP is valid");
    let addr = bech32::encode::<Bech32m>(hrp, &proto)?;

    Ok(addr)
}

/// Decode a Spark address, returning `(pubkey, invoice, signature, network)`.
///
/// # Errors
///
/// This function will return an error if:
/// * The address is not valid bech32m (`SparkAddressError::InvalidBech32`)
/// * The address has an unknown prefix (`SparkAddressError::UnknownPrefix`)
/// * The address has invalid protocol data (`SparkAddressError::BadProto`)
/// * The public key length is not 33 bytes (`SparkAddressError::WrongKeyLength`)
/// * The public key is invalid secp256k1 (when `validate-secp256k1` feature is enabled) (`SparkAddressError::InvalidSecp256k1`)
pub fn decode_spark_address(addr: &str) -> Result<SparkAddressData, SparkAddressError> {
    let has_upper = addr.bytes().any(|b| b.is_ascii_uppercase());
    let has_lower = addr.bytes().any(|b| b.is_ascii_lowercase());
    if has_upper && has_lower {
        return Err(SparkAddressError::MixedCase);
    }

    let (hrp, proto) = bech32::decode(addr)?;

    // The Bech32 spec requires the HRP to be lowercase. The `bech32`
    // crate accepts uppercase HRPs, so we enforce the stricter rule
    // here.
    let hrp_str = hrp.to_string();
    if hrp_str.bytes().any(|b| b.is_ascii_uppercase()) {
        return Err(SparkAddressError::MixedCase);
    }

    // Reject legacy Bech32 (BIP-173) by re-encoding with Bech32m and
    // comparing the checksum. If it differs, the original variant must
    // have been classic Bech32.
    let reencoded = bech32::encode::<Bech32m>(hrp, &proto)?;
    if reencoded.to_lowercase() != addr.to_lowercase() {
        return Err(SparkAddressError::InvalidVariant);
    }

    let network = Network::from_hrp(&hrp_str).ok_or_else(|| SparkAddressError::UnknownPrefix(hrp_str.clone()))?;

    let data = decode_proto(&proto)?;

    if data.key.len() != 33 {
        return Err(SparkAddressError::WrongKeyLength(data.key.len()));
    }

    let hex_key = bytes_to_hex(data.key);

    #[cfg(feature = "validate-secp256k1")]
    validate_pubkey(&hex_key)?;

    Ok(SparkAddressData {
        identity_public_key: hex_key,
        invoice: data.invoice_fields,
        signature: data.signature,
        network,
    })
}

/* ------------------------------------------------------------- *
 *  (feature-gated) secp256k1 validation                               *
 * ------------------------------------------------------------- */

#[cfg(feature = "validate-secp256k1")]
fn validate_pubkey(hex_str: &str) -> Result<(), SparkAddressError> {
    use secp256k1::PublicKey;
    let bytes = hex_to_bytes(hex_str)?;
    PublicKey::from_slice(&bytes).map_err(|_| SparkAddressError::InvalidSecp256k1)?;
    Ok(())
}

#[cfg(not(feature = "validate-secp256k1"))]
fn _validate_pubkey(_: &str) {}

/* ------------------------------------------------------------- *
 *  Tests                                                         *
 * ------------------------------------------------------------- */

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::secp256k1::PublicKey;
    use chrono::{DateTime, NaiveDateTime, Utc};
    use core::str::FromStr;
    use lazy_static::lazy_static;
    use spark_invoice::{PaymentType, SatsPayment, TokensPayment};
    use token_identifier::TokenIdentifier;
    use uuid::Uuid;

    const PUBKEY: &str = "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
    const MAINNET_ADDRESS: &str = "sp1pgssy7d7vel0nh9m4326qc54e6rskpczn07dktww9rv4nu5ptvt0s9ucez8h3s";
    const REGTEST_ADDRESS: &str = "sprt1pgssy7d7vel0nh9m4326qc54e6rskpczn07dktww9rv4nu5ptvt0s9ucd5rgc0";

    const REGTEST_INVOICE_ADDRESS: &str = "sprt1pgss9n9jdwnecca27cxfryhasa97xzr6arv8qvn4mu89tpcy5mf6fufjzf6ssqgjzqqe352f0zv8wyvjfjdp9drc3lsj5pnd09xk2mt0xgss9n9jdwnecca27cxfryhasa97xzr6arv8qvn4mu89tpcy5mf6fufj8gxq3ad25mzsvyxq3jkmxqc6yc9zpd3qfcplvq92wnuz3t25djj9rerff338xweffdn69406e24klz53zgpq86qcqsq08";

    lazy_static! {
        pub static ref TEST_UUID: Uuid = Uuid::now_v7();
        pub static ref TOKEN_IDENTIFIER_STR: &'static str =
            "btknrt1kcsyuqlkqz48f7pg442xefz3u355ccnn8v55keaz6hav42m032gs5nly6r";
        pub static ref TOKEN_IDENTIFIER: TokenIdentifier =
            TokenIdentifier::decode_bech32m(&TOKEN_IDENTIFIER_STR, bitcoin::network::Network::Regtest,)
                .expect("decode token identifier");
        pub static ref SATS_AMOUNT: u64 = 1000;
        pub static ref TOKEN_AMOUNT: u128 = 1000;
        pub static ref EXPIRY_TIME: DateTime<Utc> = Utc::now() + chrono::Duration::hours(24);
        pub static ref MEMO: &'static str = "myMemo";
        pub static ref SCHNORR_SIGNATURE_BYTES: Vec<u8> = vec![0x11; 64];
    }

    #[test]
    fn mainnet_round_trip() {
        let data = SparkAddressData {
            identity_public_key: PUBKEY.into(),
            invoice: None,
            signature: None,
            network: Network::Mainnet,
        };
        let encoded = encode_spark_address(data.clone()).unwrap();
        assert_eq!(encoded, MAINNET_ADDRESS);
        let decoded = decode_spark_address(&encoded).unwrap();
        assert_eq!(decoded, data);

        let decoded = decode_spark_address(MAINNET_ADDRESS).unwrap();
        assert_eq!(decoded.network, Network::Mainnet);
        assert_eq!(decoded.identity_public_key, PUBKEY);
    }

    #[test]
    fn regtest_round_trip() {
        let data = SparkAddressData {
            identity_public_key: PUBKEY.into(),
            invoice: None,
            signature: None,
            network: Network::Regtest,
        };
        let encoded = encode_spark_address(data.clone()).unwrap();
        assert_eq!(encoded, REGTEST_ADDRESS);
        let decoded = decode_spark_address(&encoded).unwrap();
        assert_eq!(decoded, data);

        let decoded = decode_spark_address(MAINNET_ADDRESS).unwrap();
        assert_eq!(decoded.network, Network::Mainnet);
        assert_eq!(decoded.identity_public_key, PUBKEY);
    }

    #[test]
    fn regtest_token_invoice_encode_decode() {
        let payment_type = PaymentType::Tokens(TokensPayment {
            token_identifier: *TOKEN_IDENTIFIER,
            amount: *TOKEN_AMOUNT,
        });

        let invoice_fields = SparkInvoiceFields {
            version: 1,
            id: *TEST_UUID,
            payment_type,
            memo: Some(MEMO.to_string()),
            sender_public_key: Some(PublicKey::from_str(PUBKEY).unwrap()),
            expiry_time: Some(*EXPIRY_TIME),
        };

        let data = SparkAddressData {
            identity_public_key: PUBKEY.into(),
            invoice: Some(invoice_fields),
            signature: Some(SCHNORR_SIGNATURE_BYTES.clone()),
            network: Network::Regtest,
        };

        let encoded = encode_spark_address(data.clone()).unwrap();
        let decoded = decode_spark_address(&encoded).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn regtest_btc_invoice_encode_decode() {
        let payment_type = PaymentType::Sats(SatsPayment { amount: *SATS_AMOUNT });

        let invoice_fields = SparkInvoiceFields {
            version: 1,
            id: *TEST_UUID,
            payment_type,
            memo: Some(MEMO.to_string()),
            sender_public_key: Some(PublicKey::from_str(PUBKEY).unwrap()),
            expiry_time: Some(*EXPIRY_TIME),
        };

        let data = SparkAddressData {
            identity_public_key: PUBKEY.into(),
            invoice: Some(invoice_fields),
            signature: Some(SCHNORR_SIGNATURE_BYTES.clone()),
            network: Network::Regtest,
        };

        let encoded = encode_spark_address(data.clone()).unwrap();
        let decoded = decode_spark_address(&encoded).unwrap();
        assert_eq!(decoded, data);
    }
}
