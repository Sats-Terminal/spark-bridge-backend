use bitcoin::Network;
use bitcoin::hashes::Hash;
use bitcoin::hashes::sha256::Hash as Sha256Hash;
use serde::{Deserialize, Serialize};
use spark_address::Network as SparkNetwork;
use token_identifier::TokenIdentifier;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WRunesMetadata {
    pub token_identifier: TokenIdentifier,
    pub token_name: String,
    pub token_ticker: String,
}

pub fn create_wrunes_metadata(rune_id: String) -> WRunesMetadata {
    let rune_id_hash = Sha256Hash::hash(rune_id.as_bytes());
    let token_identifier = TokenIdentifier::new(rune_id_hash);
    let token_name = rune_id.clone();
    let token_ticker = "ticker".to_string();

    WRunesMetadata {
        token_identifier,
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
