use bitcoin::{Address, Network};
use btc_indexer_config::{IndexerClientConfig, MaestroClientConfig};
use global_utils::logger::init_logger;
use ordinals::RuneId;
use std::{env, str::FromStr, time::Duration};
use tests::{
    bitcoin_client::{BitcoinClient, BitcoinTestnetClient},
    constants::{DEFAULT_FAUCET_AMOUNT, PAYING_INPUT_SATS_AMOUNT},
    gateway_client::*,
    rune_manager::setup_rune_manager,
    spark_client::{SparkClient, SparkClientConfig},
    user_wallet::{TransferType, UserWallet},
};
use tokio::time::sleep;
use url::Url;

#[tokio::test]
async fn test_spark_testnet4() {
    let _guard = init_logger();
    let network = Network::Testnet4;
    let testnet4_key = "a800a85be33a66ea91bc08b5b34494987a8fe8933b7d7a0deb2bfee15aca0883";
    let testnet_rune_id = Some(RuneId::new(108272, 144).unwrap());
    // Setup

    let esplora_url = Url::parse("https://mempool.space/testnet4/api/").unwrap();
    tracing::info!("Start setup");

    let gateway_client = GatewayClient::new(GatewayConfig {
        address: "http://localhost:8060".parse().unwrap(),
    });

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

    let mut bitcoin_client = BitcoinTestnetClient::new(
        esplora_url,
        IndexerClientConfig::Maestro(MaestroClientConfig {
            url: Url::parse("https://xbt-testnet.gomaestro-api.org/v0/").unwrap(),
            key: env::var("MAESTRO_API_KEY").expect("MAESTRO_API_KEY environment variable not set"),
            confirmation_threshold: 0,
        }),
    );

    let (rune_manager, transaction) =
        setup_rune_manager(&mut bitcoin_client, network, Some(testnet4_key), testnet_rune_id).await;
    bitcoin_client.broadcast_transaction(transaction.clone()).await.unwrap();

    let rune_id = rune_manager.get_rune_id();
    let mut user_wallet = UserWallet::new(spark_client.clone(), rune_id, network, Some(testnet4_key))
        .await
        .unwrap();
    bitcoin_client
        .faucet(user_wallet.get_address(), DEFAULT_FAUCET_AMOUNT)
        .await
        .unwrap();

    tracing::info!("Setup finished");

    // Mint runes for user wallet

    let utxos_data = bitcoin_client
        .get_address_data(user_wallet.get_address())
        .await
        .unwrap();
    let transaction = rune_manager
        .build_mint_tx(user_wallet.get_address(), utxos_data)
        .await
        .unwrap();
    bitcoin_client.broadcast_transaction(transaction.clone()).await.unwrap();

    let utxos_data = bitcoin_client
        .get_address_data(user_wallet.get_address())
        .await
        .unwrap();
    let transaction = user_wallet.build_unite_unspent_utxos_tx(&utxos_data).await.unwrap();
    bitcoin_client.broadcast_transaction(transaction.clone()).await.unwrap();

    let utxos_data = bitcoin_client
        .get_address_data(user_wallet.get_address())
        .await
        .unwrap();
    let rune_balance = user_wallet.get_rune_balance(&utxos_data).await.unwrap();
    assert!(rune_balance > 0, "Rune balance should be greater than 0");

    let deposit_amount = 100_000;

    let get_runes_deposit_address_request = GetRunesDepositAddressRequest {
        user_id: user_wallet.get_user_id(),
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

    let deposit_address = Address::from_str(&get_runes_deposit_address_response.address)
        .unwrap()
        .assume_checked();
    let utxos_data = bitcoin_client
        .get_address_data(user_wallet.get_address())
        .await
        .unwrap();
    let transaction = user_wallet
        .build_transfer_tx(
            TransferType::RuneTransfer {
                rune_amount: deposit_amount,
            },
            deposit_address,
            &utxos_data,
        )
        .await
        .unwrap();
    bitcoin_client.broadcast_transaction(transaction.clone()).await.unwrap();
    let txid = transaction.compute_txid();

    tracing::info!("Transfer txid: {:?}", txid);

    let spark_address = user_wallet.get_spark_address().unwrap();

    tracing::info!("spark_address: {:?}", spark_address);

    let bridge_runes_request = BridgeRunesSparkRequest {
        btc_address: get_runes_deposit_address_response.address,
        bridge_address: spark_address.clone(),
        txid: txid.to_string(),
        vout: 1,
        fee_payment: None,
    };

    let bridge_runes_response = gateway_client.bridge_runes(bridge_runes_request).await.unwrap();
    tracing::info!("bridge_runes_response: {:?}", bridge_runes_response);

    tracing::debug!("Waiting 30s to process bridge request");
    sleep(Duration::from_secs(30)).await;

    // get spark deposit address

    let spark_deposit_amount = 50_000;

    tracing::info!("Getting spark deposit address");

    let spark_deposit_address_request = GetSparkDepositAddressRequest {
        user_id: user_wallet.get_user_id(),
        rune_id: rune_id.to_string(),
        amount: spark_deposit_amount,
    };

    let spark_deposit_address = gateway_client
        .get_spark_deposit_address(spark_deposit_address_request)
        .await
        .unwrap()
        .address;

    tracing::info!("Spark deposit address: {:?}", spark_deposit_address);

    tracing::info!("Transferring spark to deposit address");

    user_wallet
        .transfer_spark(spark_deposit_amount, spark_deposit_address.clone())
        .await
        .unwrap();

    tracing::info!("Spark transferred to deposit address");

    // exit spark request

    tracing::info!("Starting exit spark flow");

    let utxos_data = bitcoin_client
        .get_address_data(user_wallet.get_address())
        .await
        .unwrap();
    let transaction = user_wallet
        .build_transfer_tx(
            TransferType::BtcTransfer {
                sats_amount: PAYING_INPUT_SATS_AMOUNT,
            },
            user_wallet.get_address(),
            &utxos_data,
        )
        .await
        .unwrap();
    bitcoin_client.broadcast_transaction(transaction.clone()).await.unwrap();

    let paying_input = user_wallet
        .create_user_paying_transfer_input(transaction)
        .await
        .unwrap();

    tracing::info!("Paying input: {:?}", paying_input);

    let exit_spark_request = ExitSparkRequest {
        spark_address: spark_deposit_address.clone(),
        paying_input: paying_input,
        fee_payment: None,
    };

    gateway_client.exit_spark(exit_spark_request).await.unwrap();

    tracing::info!("Finish exiting spark flow");

    sleep(Duration::from_secs(5)).await;

    // check balance

    let utxos_data = bitcoin_client
        .get_address_data(user_wallet.get_address())
        .await
        .unwrap();
    let balance = user_wallet.get_rune_balance(&utxos_data).await.unwrap();
    tracing::info!("Balance: {:?}", balance);

    assert_eq!(balance, deposit_amount, "Balance should be equal to deposit amount");
}
