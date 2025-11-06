use bitcoin::Network;
use btc_indexer_config::{IndexerClientConfig, TitanClientConfig};
use global_utils::logger::init_logger;
use tests::{
    bitcoin_client::{BitcoinClient, BitcoinClientConfig, BitcoinRegtestClient},
    constants::{BLOCKS_TO_GENERATE, DEFAULT_FAUCET_AMOUNT},
    rune_manager::setup_rune_manager,
    spark_client::{SparkClient, SparkClientConfig},
    user_wallet::{TransferType, UserWallet},
    utils::create_credentials,
};

#[tokio::test]
async fn test_rune_manager() {
    let _guard = init_logger();
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

    let spark_client = SparkClient::new(
        SparkClientConfig {
            coordinator_url: "https://2.spark.flashnet.xyz".to_string(),
            operator_public_keys: vec![
                "03dfbdff4b6332c220f8fa2ba8ed496c698ceada563fa01b67d9983bfc5c95e763".to_string(),
                "03e625e9768651c9be268e287245cc33f96a68ce9141b0b4769205db027ee8ed77".to_string(),
                "022eda13465a59205413086130a65dc0ed1b8f8e51937043161f8be0c369b1a410".to_string(),
            ],
            certificate_path: "../infrastructure/configurations/certificates/Flashnet-CA.pem".to_string(),
        },
        network,
    )
    .await
    .unwrap();

    let (rune_manager, transaction) = setup_rune_manager(&mut bitcoin_client, network, None, None).await;
    bitcoin_client.broadcast_transaction(transaction).await.unwrap();
    bitcoin_client.generate_blocks(BLOCKS_TO_GENERATE, None).await.unwrap();

    let rune_id = rune_manager.get_rune_id();

    let mut user_wallet = UserWallet::new(spark_client, rune_id, network, None).await.unwrap();
    bitcoin_client
        .faucet(user_wallet.get_address(), DEFAULT_FAUCET_AMOUNT)
        .await
        .unwrap();
    let utxos_data = bitcoin_client
        .get_address_data(rune_manager.get_p2tr_address())
        .await
        .unwrap();
    let transaction = rune_manager
        .build_mint_tx(user_wallet.get_address(), utxos_data)
        .await
        .unwrap();
    bitcoin_client.broadcast_transaction(transaction).await.unwrap();
    bitcoin_client.generate_blocks(BLOCKS_TO_GENERATE, None).await.unwrap();

    let utxos_data = bitcoin_client
        .get_address_data(user_wallet.get_address())
        .await
        .unwrap();
    let transaction = user_wallet.build_unite_unspent_utxos_tx(&utxos_data).await.unwrap();
    bitcoin_client.broadcast_transaction(transaction).await.unwrap();
    bitcoin_client.generate_blocks(BLOCKS_TO_GENERATE, None).await.unwrap();

    let utxos_data = bitcoin_client
        .get_address_data(user_wallet.get_address())
        .await
        .unwrap();
    let rune_balance = user_wallet.get_rune_balance(&utxos_data).await.unwrap();
    assert!(rune_balance > 0, "Rune balance should be greater than 0");

    let dummy_address = create_credentials(network, None).0;
    let transfer_amount = 1000;
    let utxos_data = bitcoin_client
        .get_address_data(user_wallet.get_address())
        .await
        .unwrap();
    let transaction = user_wallet
        .build_transfer_tx(
            TransferType::RuneTransfer {
                rune_amount: transfer_amount,
            },
            dummy_address.clone(),
            &utxos_data,
        )
        .await
        .unwrap();
    bitcoin_client.broadcast_transaction(transaction).await.unwrap();
    bitcoin_client.generate_blocks(BLOCKS_TO_GENERATE, None).await.unwrap();

    let utxos_data = bitcoin_client.get_address_data(dummy_address).await.unwrap();

    tracing::info!("Address rune utxos: {:?}", utxos_data);
    let rune_utxo = utxos_data[0].clone();
    assert_eq!(rune_utxo.runes[0].rune_id.to_string(), rune_id.to_string());
    assert_eq!(rune_utxo.runes[0].amount as u64, transfer_amount);
}
