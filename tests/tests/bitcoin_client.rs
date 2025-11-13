mod common;

use std::str::FromStr;

use bitcoin::{Address, Network};
use btc_indexer_config::{IndexerClientConfig, TitanClientConfig};
use common::bitcoin_client::{BitcoinClient, BitcoinClientConfig, BitcoinRegtestClient};
use tokio;

#[tokio::test]
async fn test_get_address_data() {
    let network = Network::Regtest;

    let mut bitcoin_client = BitcoinRegtestClient::new(
        BitcoinClientConfig {
            url: "http://127.0.0.1:18443".to_string(),
            username: "bitcoin".to_string(),
            password: "bitcoinpass".to_string(),
        },
        IndexerClientConfig::Titan(TitanClientConfig {
            url: "http://127.0.0.1:3030".to_string(),
        }),
    )
    .await
    .unwrap();

    let address = Address::from_str("bcrt1qsvju8jnhu5any3ymanax4zuppzk80yj24d6u8r")
        .unwrap()
        .require_network(network)
        .unwrap();

    bitcoin_client.faucet(address.clone(), 100000000).await.unwrap();

    let address_data = bitcoin_client.get_address_data(address).await.unwrap();
    println!("address_data: {:?}", address_data);
}
