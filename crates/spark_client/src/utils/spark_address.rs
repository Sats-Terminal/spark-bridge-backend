use bech32::Bech32m;
use hex;
use crate::common::error::SparkClientError;

// FIXME: ensure proper sequence of network types
/// Networks supported by Spark.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Network {
    /// Main Bitcoin network (`sp` prefix).
    Mainnet,
    /// Bitcoin testnet (`spt` prefix).
    Testnet,
    /// Regression-test network (`sprt` prefix).
    Regtest,
    /// Signet (`sps` prefix).
    Signet,
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

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SparkAddressData {
    /// Compressed secp256k1 public key, hex-encoded (`02/03 + 32 bytes`).
    pub identity_public_key: String,
    /// Network for which the address is intended (determines HRP prefix).
    pub network: Network,
}

fn decode_proto(buf: &[u8]) -> Result<&[u8], SparkClientError> {
    const TAG: u8 = 0x0a;
    if buf.len() >= 3 && buf[0] == TAG && buf[1] as usize + 2 == buf.len() {
        Ok(&buf[2..])
    } else {
        Err(SparkClientError::DecodeError("decode_proto error: Bad proto".to_string()))
    }
}

pub fn decode_spark_address(addr: String) -> Result<SparkAddressData, SparkClientError> {
    // -----------------------------------------------------------------
    // Early sanity checks (avoid allocating in `bech32::decode` when we
    // already know the string is invalid).
    // -----------------------------------------------------------------
    if addr.len() > 90 {
        return Err(SparkClientError::DecodeError(
            "decode_spark_address error: Invalid length".to_string(),
        ));
    }

    let has_upper = addr.bytes().any(|b| b.is_ascii_uppercase());
    let has_lower = addr.bytes().any(|b| b.is_ascii_lowercase());
    if has_upper && has_lower {
        return Err(SparkClientError::DecodeError(
            "decode_spark_address error: Mixed case".to_string(),
        ));
    }

    let (hrp, proto) = bech32::decode(&addr).map_err(|e| SparkClientError::DecodeError(e.to_string()))?;

    // The Bech32 spec requires the HRP to be lowercase. The `bech32`
    // crate accepts uppercase HRPs, so we enforce the stricter rule
    // here.
    let hrp_str = hrp.to_string();
    if hrp_str.bytes().any(|b| b.is_ascii_uppercase()) {
        return Err(SparkClientError::DecodeError(
            "decode_spark_address error: Mixed case".to_string(),
        ));
    }

    // Reject legacy Bech32 (BIP-173) by re-encoding with Bech32m and
    // comparing the checksum. If it differs, the original variant must
    // have been classic Bech32.
    let reencoded = bech32::encode::<Bech32m>(hrp, &proto).map_err(|e| SparkClientError::DecodeError(e.to_string()))?;
    if reencoded.to_lowercase() != addr.to_lowercase() {
        return Err(SparkClientError::DecodeError(
            "decode_spark_address error: Invalid variant".to_string(),
        ));
    }

    let network = Network::from_hrp(&hrp_str)
        .ok_or_else(|| SparkClientError::DecodeError(format!("decode_spark_address error: Unknown prefix: {}", hrp_str)))?;

    let key = decode_proto(&proto).map_err(|e| SparkClientError::DecodeError(e.to_string()))?;

    if key.len() != 33 {
        return Err(SparkClientError::DecodeError(format!(
            "decode_spark_address error: Wrong key length: {}",
            key.len()
        )));
    }

    let hex_key = hex::encode(key);

    Ok(SparkAddressData {
        identity_public_key: hex_key,
        network,
    })
}
