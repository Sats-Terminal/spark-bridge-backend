use std::env;
use std::str::FromStr;
use ordinals::RuneId;
use std::time::Duration;
use tokio::time::sleep;
use tracing;
use url::Url;
use tests::bitcoin_client::{BitcoinClient, BitcoinClientConfig};
use tests::error::RuneError;
use tests::gateway_client::{BridgeRunesSparkRequest, ExitSparkRequest, GatewayClient, GatewayConfig, GetRunesDepositAddressRequest};
use tests::rune_manager::RuneManager;
use tests::spark_client::{SparkClient, SparkClientConfig};
use tests::user_wallet::{TransferType, UserWallet};

async fn create_bitcoin_client() -> Result<BitcoinClient, RuneError> {
    dotenvy::dotenv().ok();

    let bitcoin_host = env::var("BITCOIN_RPC_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let bitcoin_port = env::var("BITCOIN_RPC_PORT").unwrap_or_else(|_| "18443".to_string());
    let bitcoin_username = env::var("BITCOIN_RPC_USERNAME").unwrap_or_else(|_| "bitcoin".to_string());
    let bitcoin_password = env::var("BITCOIN_RPC_PASSWORD").unwrap_or_else(|_| "bitcoinpass".to_string());
    let titan_url = env::var("TITAN_URL").unwrap_or_else(|_| "http://127.0.0.1:3030".to_string());

    let bitcoin_url = format!("http://{}:{}", bitcoin_host, bitcoin_port);

    let config = BitcoinClientConfig {
        bitcoin_url,
        titan_url,
        bitcoin_username,
        bitcoin_password,
    };

    BitcoinClient::new(config).map_err(RuneError::BitcoinClientError)
}

async fn create_spark_client() -> Result<SparkClient, RuneError> {
    let config = SparkClientConfig {
        coordinator_url: "https://localhost:50051".to_string(),
        certificate_path: "./certs/ca.pem".to_string(),
        operator_public_keys: vec![
            "02abc123...".to_string(),
        ],
    };
    SparkClient::new(config)
        .await
        .map_err(|e| RuneError::SparkClientError(e))
}

fn create_gateway_client() -> Result<GatewayClient, RuneError> {
    let config = GatewayConfig {
        address: Url::parse("http://localhost:3000")
            .map_err(|e| RuneError::InvalidData(format!("Failed to parse gateway URL: {}", e)))?,
    };
    Ok(GatewayClient::new(config))
}

async fn create_user_wallet(
    bitcoin_client: BitcoinClient,
    spark_client: SparkClient,
    rune_id: RuneId,
) -> Result<UserWallet, RuneError> {
    UserWallet::new(bitcoin_client, spark_client, rune_id).await
}

async fn mint_runes_to_user(
    rune_manager: &mut RuneManager,
    user: &UserWallet,
) -> Result<(), RuneError> {
    tracing::info!("Minting runes to user: {}", user.get_address());
    rune_manager.mint_rune(user.get_address()).await?;
    sleep(Duration::from_secs(2)).await;
    Ok(())
}

#[tokio::test]
async fn test_five_users_bridge_no_wrapped_trade() -> Result<(), RuneError> {
    tracing::info!("Starting Test Flow 1: 5 Users Bridge (No Wrapped Trade)");

    let bitcoin_client = create_bitcoin_client().await?;
    let spark_client = create_spark_client().await?;
    let gateway_client = create_gateway_client()?;

    let mut rune_manager = RuneManager::new(bitcoin_client.clone()).await?;
    let rune_id = rune_manager.get_rune_id().await;
    tracing::info!("Rune ID: {}", rune_id);

    let mut users = Vec::new();
    for i in 0..5 {
        tracing::info!("Creating user {}", i + 1);
        let user = create_user_wallet(
            bitcoin_client.clone(),
            spark_client.clone(),
            rune_id,
        )
            .await?;
        users.push(user);
    }

    for (i, user) in users.iter_mut().enumerate() {
        tracing::info!("\nProcessing User {}", i + 1);

        let btc_address = user.get_address();
        tracing::info!("User {} BTC Address: {}", i + 1, btc_address);

        mint_runes_to_user(&mut rune_manager, user).await?;

        let rune_balance = user.get_rune_balance().await?;
        tracing::info!("User {} Rune Balance: {}", i + 1, rune_balance);

        let bridge_amount = 50_000u64;
        let deposit_address_response = gateway_client
            .get_runes_deposit_address(GetRunesDepositAddressRequest {
                user_id: user.get_user_id(),
                rune_id: rune_id.to_string(),
                amount: bridge_amount,
            })
            .await
            .map_err(|e| RuneError::InvalidData(format!("Failed to get deposit address: {:?}", e)))?;

        tracing::info!("User {} Deposit Address: {}", i + 1, deposit_address_response.address);

        let bridge_address = bitcoin::Address::from_str(&deposit_address_response.address)
            .map_err(|e| RuneError::InvalidData(format!("Invalid bridge address: {}", e)))?
            .require_network(bitcoin::Network::Regtest)
            .map_err(|e| RuneError::InvalidData(format!("Invalid network: {}", e)))?;

        let tx = user
            .transfer(
                TransferType::RuneTransfer {
                    rune_amount: bridge_amount,
                },
                bridge_address.clone(),
            )
            .await?;

        let txid = tx.compute_txid();
        tracing::info!("User {} Transfer TXID: {}", i + 1, txid);

        gateway_client
            .bridge_runes(BridgeRunesSparkRequest {
                btc_address: btc_address.to_string(),
                bridge_address: bridge_address.to_string(),
                txid: txid.to_string(),
                vout: 1,
            })
            .await
            .map_err(|e| RuneError::InvalidData(format!("Failed to bridge runes: {:?}", e)))?;

        tracing::info!("User {} successfully bridged {} runes", i + 1, bridge_amount);
        sleep(Duration::from_secs(2)).await;
    }

    tracing::info!("\nTest Flow 1 Completed Successfully");
    Ok(())
}

#[tokio::test]
async fn test_five_users_bridge_with_wrapped_trade() -> Result<(), RuneError> {
    tracing::info!("Starting Test Flow 2: 5 Users Bridge + Wrapped Trade");

    let bitcoin_client = create_bitcoin_client().await?;
    let spark_client = create_spark_client().await?;
    let gateway_client = create_gateway_client()?;

    let mut rune_manager = RuneManager::new(bitcoin_client.clone()).await?;
    let rune_id = rune_manager.get_rune_id().await;
    tracing::info!("Rune ID: {}", rune_id);

    let mut users = Vec::new();
    for i in 0..5 {
        tracing::info!("Creating user {}", i + 1);
        let user = create_user_wallet(
            bitcoin_client.clone(),
            spark_client.clone(),
            rune_id,
        )
            .await?;
        users.push(user);
    }

    for (i, user) in users.iter_mut().enumerate() {
        tracing::info!("\nBridging for User {}", i + 1);

        mint_runes_to_user(&mut rune_manager, user).await?;

        let btc_address = user.get_address();
        let rune_balance = user.get_rune_balance().await?;
        tracing::info!("User {} Rune Balance: {}", i + 1, rune_balance);

        let bridge_amount = 100_000u64;
        let deposit_address_response = gateway_client
            .get_runes_deposit_address(GetRunesDepositAddressRequest {
                user_id: user.get_user_id(),
                rune_id: rune_id.to_string(),
                amount: bridge_amount,
            })
            .await
            .map_err(|e| RuneError::InvalidData(format!("Failed to get deposit address: {:?}", e)))?;

        let bridge_address = bitcoin::Address::from_str(&deposit_address_response.address)
            .map_err(|e| RuneError::InvalidData(format!("Invalid bridge address: {}", e)))?
            .require_network(bitcoin::Network::Regtest)
            .map_err(|e| RuneError::InvalidData(format!("Invalid network: {}", e)))?;

        let tx = user
            .transfer(
                TransferType::RuneTransfer {
                    rune_amount: bridge_amount,
                },
                bridge_address.clone(),
            )
            .await?;

        let txid = tx.compute_txid();

        gateway_client
            .bridge_runes(BridgeRunesSparkRequest {
                btc_address: btc_address.to_string(),
                bridge_address: bridge_address.to_string(),
                txid: txid.to_string(),
                vout: 1,
            })
            .await
            .map_err(|e| RuneError::InvalidData(format!("Failed to bridge runes: {:?}", e)))?;

        tracing::info!("User {} bridged {} runes", i + 1, bridge_amount);
        sleep(Duration::from_secs(3)).await;
    }

    tracing::info!("\nStarting Wrapped Rune Trading");

    tracing::info!("User 1 sending 20,000 wrapped runes to User 2");
    let addr2 = users[1].get_spark_address()?;
    users[0].transfer_spark(20_000, addr2).await?;
    sleep(Duration::from_secs(2)).await;

    tracing::info!("User 2 sending 30,000 wrapped runes to User 3");
    let addr3 = users[2].get_spark_address()?;
    users[1].transfer_spark(30_000, addr3).await?;
    sleep(Duration::from_secs(2)).await;

    tracing::info!("User 3 sending 15,000 wrapped runes to User 4");
    let addr4 = users[3].get_spark_address()?;
    users[2].transfer_spark(15_000, addr4).await?;
    sleep(Duration::from_secs(2)).await;

    tracing::info!("User 4 sending 10,000 wrapped runes to User 5");
    let addr5 = users[4].get_spark_address()?;
    users[3].transfer_spark(10_000, addr5).await?;
    sleep(Duration::from_secs(2)).await;

    tracing::info!("User 5 sending 5,000 wrapped runes back to User 1");
    let addr1 = users[0].get_spark_address()?;
    users[4].transfer_spark(5_000, addr1).await?;
    sleep(Duration::from_secs(2)).await;

    tracing::info!("\nTest Flow 2 Completed Successfully");
    Ok(())
}

#[tokio::test]
async fn test_bridge_spark_transfer_exit() -> Result<(), RuneError> {
    tracing::info!("Starting Test Flow 3: Bridge -> Transfer -> Exit");

    let bitcoin_client = create_bitcoin_client().await?;
    let spark_client = create_spark_client().await?;
    let gateway_client = create_gateway_client()?;

    let mut rune_manager = RuneManager::new(bitcoin_client.clone()).await?;
    let rune_id = rune_manager.get_rune_id().await;
    tracing::info!("Rune ID: {}", rune_id);

    tracing::info!("Creating User A");
    let mut user_a = create_user_wallet(
        bitcoin_client.clone(),
        spark_client.clone(),
        rune_id,
    )
        .await?;

    tracing::info!("Creating User B");
    let mut user_b = create_user_wallet(
        bitcoin_client.clone(),
        spark_client.clone(),
        rune_id,
    )
        .await?;

    tracing::info!("\nStep 1: User A bridges 100 runes");
    mint_runes_to_user(&mut rune_manager, &user_a).await?;

    let user_a_btc_address = user_a.get_address();
    let user_a_balance = user_a.get_rune_balance().await?;
    tracing::info!("User A Rune Balance: {}", user_a_balance);

    let bridge_amount = 100_000u64;
    let deposit_address_response = gateway_client
        .get_runes_deposit_address(GetRunesDepositAddressRequest {
            user_id: user_a.get_user_id(),
            rune_id: rune_id.to_string(),
            amount: bridge_amount,
        })
        .await
        .map_err(|e| RuneError::InvalidData(format!("Failed to get deposit address: {:?}", e)))?;

    let bridge_address = bitcoin::Address::from_str(&deposit_address_response.address)
        .map_err(|e| RuneError::InvalidData(format!("Invalid bridge address: {}", e)))?
        .require_network(bitcoin::Network::Regtest)
        .map_err(|e| RuneError::InvalidData(format!("Invalid network: {}", e)))?;

    let tx = user_a
        .transfer(
            TransferType::RuneTransfer {
                rune_amount: bridge_amount,
            },
            bridge_address.clone(),
        )
        .await?;

    let txid = tx.compute_txid();
    tracing::info!("User A Transfer TXID: {}", txid);

    gateway_client
        .bridge_runes(BridgeRunesSparkRequest {
            btc_address: user_a_btc_address.to_string(),
            bridge_address: bridge_address.to_string(),
            txid: txid.to_string(),
            vout: 1,
        })
        .await
        .map_err(|e| RuneError::InvalidData(format!("Failed to bridge runes: {:?}", e)))?;

    tracing::info!("User A successfully bridged {} runes", bridge_amount);
    sleep(Duration::from_secs(3)).await;

    tracing::info!("\nStep 2: User A sends 50,000 wrapped runes to User B");
    let transfer_amount = 50_000u64;
    user_a
        .transfer_spark(transfer_amount, user_b.get_spark_address()?)
        .await?;

    tracing::info!(
        "User A successfully sent {} wrapped runes to User B",
        transfer_amount
    );
    sleep(Duration::from_secs(3)).await;

    tracing::info!("\nStep 3: User B exits 25,000 wrapped runes");

    let paying_input = user_b.create_user_paying_transfer_input().await?;
    tracing::info!("User B created paying input");

    gateway_client
        .exit_spark(ExitSparkRequest {
            spark_address: user_b.get_spark_address()?,
            paying_input,
        })
        .await
        .map_err(|e| RuneError::InvalidData(format!("Failed to exit spark: {:?}", e)))?;

    tracing::info!("User B successfully initiated exit");
    sleep(Duration::from_secs(3)).await;

    tracing::info!("\nFinal State");
    let user_b_btc_balance = user_b.get_rune_balance().await?;
    tracing::info!("User B BTC Rune Balance: {}", user_b_btc_balance);

    tracing::info!("\nTest Flow 3 Completed Successfully");
    Ok(())
}

#[tokio::test]
async fn test_basic_setup() -> Result<(), RuneError> {
    tracing::info!("Testing Basic Setup");

    let bitcoin_client = create_bitcoin_client().await?;
    let spark_client = create_spark_client().await?;
    let _gateway_client = create_gateway_client()?;

    let mut rune_manager = RuneManager::new(bitcoin_client.clone()).await?;
    let rune_id = rune_manager.get_rune_id().await;
    tracing::info!("Rune ID: {}", rune_id);

    let user = create_user_wallet(bitcoin_client.clone(), spark_client, rune_id).await?;
    tracing::info!("User BTC Address: {}", user.get_address());
    tracing::info!("User Spark Address: {}", user.get_spark_address()?);

    mint_runes_to_user(&mut rune_manager, &user).await?;
    let balance = user.get_rune_balance().await?;
    tracing::info!("User Rune Balance: {}", balance);

    tracing::info!("Basic Setup Test Completed");
    Ok(())
}