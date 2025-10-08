use crate::types::{DEFAULT_DECIMALS, DEFAULT_IS_FREEZABLE, DEFAULT_MAX_SUPPLY};
use bitcoin::secp256k1::PublicKey;
use bitcoin::{Network, secp256k1};
use lrc20::token_metadata::{SPARK_CREATION_ENTITY_PUBLIC_KEY, TokenMetadata};
use serde::{Deserialize, Serialize};
use token_identifier::TokenIdentifier;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WRunesMetadata {
    pub token_identifier: TokenIdentifier,
    pub token_name: String,
    pub token_ticker: String,
}

const DEFAULT_TOKEN_TICKER: &str = "ticker";

pub fn create_wrunes_metadata(
    rune_id: String,
    issuer_public_key: PublicKey,
    network: Network,
) -> Result<WRunesMetadata, secp256k1::Error> {
    let token_name = rune_id;
    let token_ticker = DEFAULT_TOKEN_TICKER.to_string();

    let token_metadata = TokenMetadata::new(
        issuer_public_key,
        token_name.clone(),
        token_ticker.clone(),
        DEFAULT_DECIMALS,
        DEFAULT_MAX_SUPPLY,
        DEFAULT_IS_FREEZABLE,
        Some(PublicKey::from_slice(&SPARK_CREATION_ENTITY_PUBLIC_KEY)?),
        network,
    );

    Ok(WRunesMetadata {
        token_identifier: token_metadata.compute_token_identifier(),
        token_name,
        token_ticker,
    })
}
