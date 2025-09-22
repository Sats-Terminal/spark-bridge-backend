use tests::bitcoin_client::{BitcoinClient, BitcoinClientConfig};
use tokio;


#[tokio::test]
async fn test_get_address_data() {
    let config = BitcoinClientConfig {
        bitcoin_url: "http://localhost:18443".to_string(),
        titan_url: "http://localhost:3030".to_string(),
        bitcoin_username: "bitcoin".to_string(),
        bitcoin_password: "bitcoinpass".to_string(),
    };
    let bitcoin_client = BitcoinClient::new(config).unwrap();

    bitcoin_client.init_bitcoin_faucet_wallet().unwrap();
}
