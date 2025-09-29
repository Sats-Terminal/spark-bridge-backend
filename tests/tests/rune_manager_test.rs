use tests::bitcoin_client::{BitcoinClient, BitcoinClientConfig};
use tests::rune_manager::RuneManager;
use tests::user_wallet::UserWallet;
use global_utils::logger::init_logger;
use tests::utils::create_credentials;

#[tokio::test]
async fn test_rune_manager() {
    let _guard = init_logger();

    let bitcoin_client = BitcoinClient::new(
        BitcoinClientConfig {
            bitcoin_url: "http://127.0.0.1:18443".to_string(),
            titan_url: "http://127.0.0.1:3030".to_string(),
            bitcoin_username: "bitcoin".to_string(),
            bitcoin_password: "bitcoinpass".to_string(),
        }
    ).unwrap();

    let mut rune_manager = RuneManager::new(bitcoin_client.clone()).await.unwrap();
    let rune_id = rune_manager.get_rune_id().await;

    let mut user_wallet = UserWallet::new(bitcoin_client.clone(), rune_id).await.unwrap();
    rune_manager.mint_rune(user_wallet.get_address()).await.unwrap();

    user_wallet.unite_unspent_utxos().await.unwrap();
    let rune_balance = user_wallet.get_rune_balance().await.unwrap();
    assert!(rune_balance > 0, "Rune balance should be greater than 0");

    let dummy_address = create_credentials().0;
    let transfer_amount = 1000;
    user_wallet.transfer_runes(transfer_amount, dummy_address.clone()).await.unwrap();
    let address_data = bitcoin_client.get_address_data(dummy_address).await.unwrap();

    tracing::info!("Address data: {:?}", address_data);
    let output = address_data.outputs[0].clone();
    assert_eq!(output.runes[0].rune_id.to_string(), rune_id.to_string());
    assert_eq!(output.runes[0].amount as u64, transfer_amount);

}
