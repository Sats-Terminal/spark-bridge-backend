use global_utils::logger::init_logger;
use tests::bitcoin_client::{BitcoinClient, BitcoinClientConfig};
use tests::rune_manager::RuneManager;
use tests::spark_client::{SparkClient, SparkClientConfig};
use tests::user_wallet::{TransferType, UserWallet};
use tests::utils::create_credentials;

#[tokio::test]
async fn test_rune_manager() {
    let _guard = init_logger();

    let bitcoin_client = BitcoinClient::new(BitcoinClientConfig {
        bitcoin_url: "http://127.0.0.1:18443".to_string(),
        titan_url: "http://127.0.0.1:3030".to_string(),
        bitcoin_username: "bitcoin".to_string(),
        bitcoin_password: "bitcoinpass".to_string(),
    })
    .unwrap();

    let spark_client = SparkClient::new(SparkClientConfig {
        coordinator_url: "https://2.spark.flashnet.xyz".to_string(),
        operator_public_keys: vec![
            "03dfbdff4b6332c220f8fa2ba8ed496c698ceada563fa01b67d9983bfc5c95e763".to_string(),
            "03e625e9768651c9be268e287245cc33f96a68ce9141b0b4769205db027ee8ed77".to_string(),
            "022eda13465a59205413086130a65dc0ed1b8f8e51937043161f8be0c369b1a410".to_string(),
        ],
        certificate_path: "../infrastructure/configurations/certificates/Flashnet-CA.pem".to_string(),
    })
    .await
    .unwrap();

    let mut rune_manager = RuneManager::new(bitcoin_client.clone()).await.unwrap();
    let rune_id = rune_manager.get_rune_id().await;

    let mut user_wallet = UserWallet::new(bitcoin_client.clone(), spark_client, rune_id)
        .await
        .unwrap();
    rune_manager.mint_rune(user_wallet.get_address()).await.unwrap();

    user_wallet.unite_unspent_utxos().await.unwrap();
    let rune_balance = user_wallet.get_rune_balance().await.unwrap();
    assert!(rune_balance > 0, "Rune balance should be greater than 0");

    let dummy_address = create_credentials().0;
    let transfer_amount = 1000;
    user_wallet
        .transfer(
            TransferType::RuneTransfer {
                rune_amount: transfer_amount,
            },
            dummy_address.clone(),
        )
        .await
        .unwrap();
    let address_data = bitcoin_client.get_address_data(dummy_address).await.unwrap();

    tracing::info!("Address data: {:?}", address_data);
    let output = address_data.outputs[0].clone();
    assert_eq!(output.runes[0].rune_id.to_string(), rune_id.to_string());
    assert_eq!(output.runes[0].amount as u64, transfer_amount);
}
