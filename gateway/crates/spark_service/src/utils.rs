use bitcoin::Network;
use bitcoin::hashes::Hash;
use bitcoin::hashes::sha256::Hash as Sha256Hash;
use bitcoin::secp256k1::PublicKey;
use serde::{Deserialize, Serialize};
use lrc20::token_metadata::TokenMetadata;
use spark_address::Network as SparkNetwork;
use token_identifier::TokenIdentifier;
use crate::types::{DEFAULT_DECIMALS, DEFAULT_IS_FREEZABLE, DEFAULT_MAX_SUPPLY};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WRunesMetadata {
    pub token_identifier: TokenIdentifier,
    pub token_name: String,
    pub token_ticker: String,
}

pub fn create_wrunes_metadata(rune_id: String, issuer_public_key: PublicKey, entity_creation_public_key: PublicKey, network: Network) -> WRunesMetadata {
    let token_name = rune_id;
    let token_ticker = "ticker".to_string();

    let token_metadata = TokenMetadata::new(
        issuer_public_key,
        token_name.clone(),
        token_ticker.clone(),
        DEFAULT_DECIMALS,
        DEFAULT_MAX_SUPPLY,
        DEFAULT_IS_FREEZABLE,
        Some(entity_creation_public_key),
        network,
    );
    
    println!("Token metadata: {:?}", token_metadata.compute_token_identifier());

    WRunesMetadata {
        token_identifier: token_metadata.compute_token_identifier(),
        token_name,
        token_ticker,
    }
}

pub fn convert_network_to_spark_network(network: Network) -> SparkNetwork {
    match network {
        Network::Bitcoin => SparkNetwork::Mainnet,
        Network::Testnet => SparkNetwork::Testnet,
        Network::Regtest => SparkNetwork::Regtest,
        Network::Signet => SparkNetwork::Signet,
        Network::Testnet4 => SparkNetwork::Local,
    }
}

pub fn bitcoin_network_to_proto_network(network: Network) -> u32 {
    match network {
        Network::Bitcoin => 1,
        Network::Regtest => 2,
        Network::Testnet | Network::Testnet4 => 3,
        Network::Signet => 4,
    }
}

pub fn spark_network_to_proto_network(network: SparkNetwork) -> u32 {
    match network {
        SparkNetwork::Mainnet => 1,
        SparkNetwork::Regtest => 2,
        SparkNetwork::Testnet | SparkNetwork::Local => 3,
        SparkNetwork::Signet => 4,
    }
}
