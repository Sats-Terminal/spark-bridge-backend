use std::str::FromStr;

use bitcoin::{Address, Network};
use spark_address::Network as SparkNetwork;

pub fn decode_address(address: &str, network: Network) -> Result<Address, String> {
    let address = Address::from_str(address).map_err(|e| format!("Failed to decode address: {}", e))?;
    let address = address
        .require_network(network)
        .map_err(|e| format!("Failed to check network: {}", e))?;
    Ok(address)
}

pub fn convert_network_to_spark_network(network: Network) -> SparkNetwork {
    match network {
        Network::Bitcoin => SparkNetwork::Mainnet,
        Network::Testnet => SparkNetwork::Testnet,
        Network::Regtest => SparkNetwork::Regtest,
        Network::Signet => SparkNetwork::Signet,
        Network::Testnet4 => SparkNetwork::Regtest,
    }
}

pub fn bitcoin_network_to_proto_network(network: Network) -> u32 {
    match network {
        Network::Bitcoin => 1,
        Network::Testnet4 | Network::Testnet | Network::Regtest => 2,
        Network::Signet => 4,
    }
}

pub fn spark_network_to_proto_network(network: SparkNetwork) -> u32 {
    match network {
        SparkNetwork::Mainnet => 1,
        SparkNetwork::Testnet | SparkNetwork::Regtest => 2,
        SparkNetwork::Local => 3,
        SparkNetwork::Signet => 4,
    }
}
