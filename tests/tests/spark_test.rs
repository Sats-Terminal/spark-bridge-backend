use tests::gateway_client::*;
use global_utils::logger::init_logger;
use tests::bitcoin_client::{BitcoinClient, BitcoinClientConfig};
use tests::rune_manager::RuneManager;
use tests::user_wallet::UserWallet;
use bitcoin::{Address, Txid};
use std::str::FromStr;
use spark_address::{encode_spark_address, SparkAddressData};
use tests::utils::create_credentials;

#[tokio::test]
async fn test_spark() {
    let _guard = init_logger();

    // Mint runes for user wallet

    let bitcoin_client = BitcoinClient::new(
        BitcoinClientConfig {
            bitcoin_url: "http://127.0.0.1:18443".to_string(),
            titan_url: "http://127.0.0.1:3030".to_string(),
            bitcoin_username: "bitcoin".to_string(),
            bitcoin_password: "bitcoinpass".to_string(),
        }
    ).expect("bitcoin client should work");

    let mut rune_manager = RuneManager::new(bitcoin_client.clone()).await.unwrap();
    let rune_id = rune_manager.get_rune_id().await;
    let mut user_wallet = UserWallet::new(bitcoin_client.clone(), rune_id).await.unwrap();

    rune_manager.mint_rune(user_wallet.get_address()).await.unwrap();
    user_wallet.unite_unspent_utxos().await.unwrap();
    
    let rune_balance = user_wallet.get_rune_balance().await.unwrap();
    assert!(rune_balance > 0, "Rune balance should be greater than 0");

    // Get runes deposit address

    let deposit_amount = 100_000;
    
    let gateway_client = GatewayClient::new(GatewayConfig {
        address: "http://localhost:8060".parse().unwrap(),
    });

    let get_runes_deposit_address_request = GetRunesDepositAddressRequest {
        user_public_key: "038347b1f5471e28612f0324f5cf5eaa74bc1e1207ae7cdef1c69f0f1e72254d59".to_string(),
        rune_id: rune_id.to_string(),
        amount: deposit_amount,
    };

    let get_runes_deposit_address_response = gateway_client
        .get_runes_deposit_address(get_runes_deposit_address_request).await.unwrap();
    tracing::info!("get_runes_deposit_address_response: {:?}", get_runes_deposit_address_response);

    // send runes to deposit address

    let deposit_address = Address::from_str(&get_runes_deposit_address_response.address).unwrap()
        .assume_checked();
    let txid = user_wallet.transfer_runes(deposit_amount, deposit_address).await.unwrap();
    tracing::info!("txid: {:?}", txid);

    // bridge runes

    let public_key = create_credentials().1.public_key();
    let spark_address = encode_spark_address(SparkAddressData {
        identity_public_key: public_key.to_string(),
        invoice: None,
        signature: None,
        network: spark_address::Network::Regtest,
    }).unwrap();

    let bridge_runes_request = BridgeRunesSparkRequest {
        btc_address: get_runes_deposit_address_response.address,
        bridge_address: spark_address,
        txid: txid.to_string(),
        vout: 1,
    };

    let bridge_runes_response = gateway_client.bridge_runes(bridge_runes_request).await.unwrap();
    tracing::info!("bridge_runes_response: {:?}", bridge_runes_response);

    // check that runes are bridged


}
