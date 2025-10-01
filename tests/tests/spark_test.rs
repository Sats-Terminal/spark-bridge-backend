use bitcoin::Address;
use global_utils::logger::init_logger;
use std::str::FromStr;
use std::time::Duration;
use tests::bitcoin_client::{BitcoinClient, BitcoinClientConfig};
use tests::gateway_client::*;
use tests::rune_manager::RuneManager;
use tests::spark_client::{SparkClient, SparkClientConfig};
use tests::user_wallet::TransferType;
use tests::user_wallet::UserWallet;
use tokio::time::sleep;

#[tokio::test]
async fn test_spark() {
    let _guard = init_logger();

    // Setup

    tracing::info!("Start setup");

    let gateway_client = GatewayClient::new(GatewayConfig {
        address: "http://localhost:8060".parse().unwrap(),
    });

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

    let bitcoin_client = BitcoinClient::new(BitcoinClientConfig {
        bitcoin_url: "http://127.0.0.1:18443".to_string(),
        titan_url: "http://127.0.0.1:3030".to_string(),
        bitcoin_username: "bitcoin".to_string(),
        bitcoin_password: "bitcoinpass".to_string(),
    })
    .expect("bitcoin client should work");

    let mut rune_manager = RuneManager::new(bitcoin_client.clone()).await.unwrap();
    let rune_id = rune_manager.get_rune_id().await;
    let mut user_wallet = UserWallet::new(bitcoin_client.clone(), spark_client.clone(), rune_id)
        .await
        .unwrap();

    tracing::info!("Setup finished");

    // Mint runes for user wallet

    rune_manager.mint_rune(user_wallet.get_address()).await.unwrap();
    user_wallet.unite_unspent_utxos().await.unwrap();

    let rune_balance = user_wallet.get_rune_balance().await.unwrap();
    assert!(rune_balance > 0, "Rune balance should be greater than 0");

    // Get runes deposit address

    let deposit_amount = 100_000;

    let get_runes_deposit_address_request = GetRunesDepositAddressRequest {
        user_public_key: user_wallet.get_public_key().to_string(),
        rune_id: rune_id.to_string(),
        amount: deposit_amount,
    };

    let get_runes_deposit_address_response = gateway_client
        .get_runes_deposit_address(get_runes_deposit_address_request)
        .await
        .unwrap();
    tracing::info!(
        "get_runes_deposit_address_response: {:?}",
        get_runes_deposit_address_response
    );

    // send runes to deposit address

    let deposit_address = Address::from_str(&get_runes_deposit_address_response.address)
        .unwrap()
        .assume_checked();
    let txid = user_wallet
        .transfer(
            TransferType::RuneTransfer {
                rune_amount: deposit_amount,
            },
            deposit_address,
        )
        .await
        .unwrap();
    tracing::info!("txid: {:?}", txid);

    // bridge runes

    let spark_address = user_wallet.get_spark_address().unwrap();

    tracing::info!("spark_address: {:?}", spark_address);

    let bridge_runes_request = BridgeRunesSparkRequest {
        btc_address: get_runes_deposit_address_response.address,
        bridge_address: spark_address.clone(),
        txid: txid.to_string(),
        vout: 1,
    };

    let bridge_runes_response = gateway_client.bridge_runes(bridge_runes_request).await.unwrap();
    tracing::info!("bridge_runes_response: {:?}", bridge_runes_response);

    sleep(Duration::from_secs(5)).await;

    // get spark deposit address

    let spark_deposit_amount = 50_000;

    tracing::info!("Getting spark deposit address");

    let spark_deposit_address_request = GetSparkDepositAddressRequest {
        user_public_key: user_wallet.get_public_key().to_string(),
        rune_id: rune_id.to_string(),
        amount: spark_deposit_amount,
    };

    let spark_deposit_address = gateway_client
        .get_spark_deposit_address(spark_deposit_address_request)
        .await
        .unwrap()
        .address;

    tracing::info!("Spark deposit address: {:?}", spark_deposit_address);

    // send runes to spark deposit address

    tracing::info!("Transferring spark to deposit address");

    user_wallet
        .transfer_spark(spark_deposit_amount, spark_deposit_address.clone())
        .await
        .unwrap();

    tracing::info!("Spark transferred to deposit address");

    // exit spark request

    tracing::info!("Starting exit spark flow");

    let paying_input = user_wallet.create_user_paying_transfer_input().await.unwrap();

    tracing::info!("Paying input: {:?}", paying_input);

    let exit_spark_request = ExitSparkRequest {
        spark_address: spark_deposit_address.clone(),
        exit_address: user_wallet.get_address().to_string(),
        paying_input: paying_input,
    };

    gateway_client.exit_spark(exit_spark_request).await.unwrap();

    tracing::info!("Finish exiting spark flow");

    sleep(Duration::from_secs(5)).await;

    // check balance

    let balance = user_wallet.get_rune_balance().await.unwrap();
    tracing::info!("Balance: {:?}", balance);

    assert_eq!(balance, deposit_amount, "Balance should be equal to deposit amount");
}
