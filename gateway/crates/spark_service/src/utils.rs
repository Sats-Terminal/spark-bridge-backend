use crate::types::{DEFAULT_IS_FREEZABLE, DEFAULT_MAX_SUPPLY};
use bitcoin::secp256k1::PublicKey;
use bitcoin::{Network, secp256k1};
use lrc20::token_metadata::{SPARK_CREATION_ENTITY_PUBLIC_KEY, TokenMetadata};
use serde::{Deserialize, Serialize};
use spark_address::Network as SparkNetwork;
use token_identifier::TokenIdentifier;
use tracing::warn;

const FALLBACK_DECIMALS: u8 = 0;

#[derive(Debug, Clone)]
pub struct RuneTokenConfig {
    pub rune_id: String,
    pub rune_name: Option<String>,
    pub divisibility: Option<u8>,
    pub max_supply: Option<u128>,
    pub icon_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WRunesMetadata {
    pub token_identifier: TokenIdentifier,
    pub token_name: String,
    pub token_ticker: String,
    pub decimals: u8,
    pub max_supply: u128,
    pub icon_url: Option<String>,
    pub original_rune_id: String,
}

pub fn create_wrunes_metadata(
    rune_config: &RuneTokenConfig,
    issuer_public_key: PublicKey,
    network: Network,
) -> Result<WRunesMetadata, secp256k1::Error> {
    let decimals = rune_config.divisibility.unwrap_or(FALLBACK_DECIMALS);
    let token_name = sanitize_name(rune_config.rune_name.as_deref(), &rune_config.rune_id);
    let token_ticker = sanitize_ticker(&rune_config.rune_id);
    let max_supply = rune_config.max_supply.unwrap_or(DEFAULT_MAX_SUPPLY);

    let token_metadata = TokenMetadata::new(
        issuer_public_key,
        token_name.clone(),
        token_ticker.clone(),
        decimals,
        max_supply,
        DEFAULT_IS_FREEZABLE,
        Some(PublicKey::from_slice(&SPARK_CREATION_ENTITY_PUBLIC_KEY)?),
        network,
    );

    Ok(WRunesMetadata {
        token_identifier: token_metadata.compute_token_identifier(),
        token_name,
        token_ticker,
        decimals,
        max_supply,
        icon_url: rune_config.icon_url.clone(),
        original_rune_id: rune_config.rune_id.clone(),
    })
}

fn sanitize_name(original: Option<&str>, fallback_id: &str) -> String {
    let mut candidate = original
        .unwrap_or(fallback_id)
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>();
    if candidate.is_empty() {
        candidate = fallback_id.replace(':', "");
    }
    if candidate.len() < 3 {
        candidate = format!("WRN{}", fallback_id.replace(':', ""));
    }
    const MAX_LEN: usize = lrc20::token_metadata::MAX_NAME_SIZE;
    if candidate.len() > MAX_LEN {
        candidate.truncate(MAX_LEN);
    }
    candidate.make_ascii_uppercase();
    candidate
}

fn sanitize_ticker(rune_id: &str) -> String {
    const MAX_LEN: usize = lrc20::token_metadata::MAX_SYMBOL_SIZE;
    let candidate = rune_id.to_string();
    if candidate.len() <= MAX_LEN {
        return candidate;
    }

    warn!("Rune id {rune_id} exceeds Spark ticker limit; compressing to base62 representation");
    base62_encode_rune_id(rune_id)
}

fn base62_encode_rune_id(rune_id: &str) -> String {
    const ALPHABET: &[u8; 62] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
    const MAX_LEN: usize = lrc20::token_metadata::MAX_SYMBOL_SIZE;

    let parts: Vec<&str> = rune_id.split(':').collect();
    if parts.len() != 2 {
        return rune_id.chars().take(MAX_LEN).collect();
    }

    let height = parts[0].parse::<u64>().unwrap_or(0);
    let tx_index = parts[1].parse::<u64>().unwrap_or(0);
    let combined = height.saturating_mul(10_000).saturating_add(tx_index);
    if combined == 0 {
        return "000000".to_string();
    }

    let mut value = combined;
    let mut encoded = Vec::new();
    while value > 0 {
        let remainder = (value % 62) as usize;
        encoded.push(ALPHABET[remainder] as char);
        value /= 62;
    }
    while encoded.len() < MAX_LEN {
        encoded.push('0');
    }
    encoded.truncate(MAX_LEN);
    encoded.iter().rev().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::Network;
    use bitcoin::secp256k1::PublicKey;

    #[test]
    fn sanitize_name_removes_non_alphanumeric_and_uppercases() {
        let result = sanitize_name(Some("WADDLE•IN•THE•MATRIX"), "859676:1968");
        assert_eq!(result, "WADDLEINTHEMATRIX");
    }

    #[test]
    fn sanitize_ticker_compresses_long_identifier() {
        let result = sanitize_ticker("859676:1968");
        assert_eq!(result.len(), lrc20::token_metadata::MAX_SYMBOL_SIZE);
        assert_eq!(result, "9Nn8V6");
    }

    #[test]
    fn sanitize_ticker_keeps_short_identifier() {
        let result = sanitize_ticker("12345");
        assert_eq!(result, "12345");
    }

    #[test]
    fn create_wrunes_metadata_uses_divisibility_and_supply() {
        let issuer_public_key = PublicKey::from_slice(&SPARK_CREATION_ENTITY_PUBLIC_KEY).unwrap();
        let config = RuneTokenConfig {
            rune_id: "12345:6".to_string(),
            rune_name: Some("MyRune".to_string()),
            divisibility: Some(3),
            max_supply: Some(123_000_000),
            icon_url: None,
        };

        let metadata = create_wrunes_metadata(&config, issuer_public_key, Network::Bitcoin).unwrap();
        assert_eq!(metadata.token_name, "MYRUNE");
        assert_eq!(metadata.decimals, 3);
        assert_eq!(metadata.max_supply, 123_000_000);
        assert_eq!(metadata.token_identifier.to_string().len() > 0, true);
    }
}
