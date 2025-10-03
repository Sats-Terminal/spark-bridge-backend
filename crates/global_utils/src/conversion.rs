use bitcoin::{Address, Network};
use spark_address::Network as SparkNetwork;
use std::str::FromStr;

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
