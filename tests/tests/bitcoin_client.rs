use tests::bitcoin_client::{BitcoinClient, BitcoinClientConfig};
use tokio;
use bitcoin::{Address, Network};
use std::str::FromStr;


#[tokio::test]
async fn test_get_address_data() {
    let config = BitcoinClientConfig {
        bitcoin_url: "http://127.0.0.1:18443".to_string(),
        titan_url: "http://127.0.0.1:3030".to_string(),
        bitcoin_username: "bitcoin".to_string(),
        bitcoin_password: "bitcoinpass".to_string(),
    };
    let network = Network::Regtest;

    let mut bitcoin_client = BitcoinClient::new(config).unwrap();

    let address = Address::from_str("bcrt1qsvju8jnhu5any3ymanax4zuppzk80yj24d6u8r")
        .unwrap()
        .require_network(network)
        .unwrap();

    bitcoin_client.faucet(address.clone(), 100000000).unwrap();

    let address_data = bitcoin_client.get_address_data(address).await.unwrap();
    println!("address_data: {:?}", address_data);

}
