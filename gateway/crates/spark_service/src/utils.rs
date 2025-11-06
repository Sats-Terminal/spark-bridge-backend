use bitcoin::{Network, secp256k1::PublicKey};
use btc_indexer_client::{client_api::Runer, error::BtcIndexerClientError};
use lrc20::token_metadata::TokenMetadata;
use serde::{Deserialize, Serialize};
use token_identifier::TokenIdentifier;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WRunesMetadata {
    pub meta: TokenMetadata,
    pub token_identifier: TokenIdentifier,
}

pub async fn create_wrunes_metadata(
    rune_id: &str,
    issuer_public_key: PublicKey,
    network: Network,
    runer: &impl Runer,
) -> Result<WRunesMetadata, BtcIndexerClientError> {
    let token_metadata = runer.get_rune_metadata(rune_id, issuer_public_key, network).await?;
    Ok(WRunesMetadata {
        meta: token_metadata.clone(),
        token_identifier: token_metadata.compute_token_identifier(),
    })
}
