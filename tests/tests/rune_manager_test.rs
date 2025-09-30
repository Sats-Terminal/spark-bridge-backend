use tests::bitcoin_client::{BitcoinClient, BitcoinClientConfig};
use tests::rune_manager::RuneManager;
use tests::user_wallet::UserWallet;
use global_utils::logger::init_logger;
use tests::utils::create_credentials;

#[tokio::test]
async fn test_single_rune_manager() {
    let _guard = init_logger();

    let bitcoin_client = BitcoinClient::new(
        BitcoinClientConfig {
            bitcoin_url: "http://127.0.0.1:18443".to_string(),
            titan_url: "http://127.0.0.1:3030".to_string(),
            bitcoin_username: "bitcoin".to_string(),
            bitcoin_password: "bitcoinpass".to_string(),
        }
    ).unwrap();

    let mut rune_manager = RuneManager::new_with_rune(bitcoin_client.clone()).await.unwrap();
    let rune_id = rune_manager.get_rune_id().await.expect("Should have at least one rune");

    let mut user_wallet = UserWallet::new(bitcoin_client.clone(), vec![rune_id]).await.unwrap();

    rune_manager.mint_rune(rune_id, user_wallet.get_address()).await.unwrap();

    user_wallet.unite_unspent_utxos().await.unwrap();
    let rune_balance = user_wallet.get_rune_balance(&rune_id).await.unwrap();
    assert!(rune_balance > 0, "Rune balance should be greater than 0");

    let dummy_address = create_credentials().0;
    let transfer_amount = 1000;
    user_wallet.transfer_runes(rune_id, transfer_amount, dummy_address.clone()).await.unwrap();

    let address_data = bitcoin_client.get_address_data(dummy_address).await.unwrap();
    tracing::info!("Address data: {:?}", address_data);

    let output = address_data.outputs[0].clone();
    assert_eq!(output.runes[0].rune_id.to_string(), rune_id.to_string());
    assert_eq!(output.runes[0].amount as u64, transfer_amount);
}

#[tokio::test]
async fn test_multiple_runes_manager() {
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

    tracing::info!("Creating first rune...");
    let rune_id_1 = rune_manager.etch_new_rune(
        Some("TESTRUNEONE".to_string()),
        1000,
        1_000_000
    ).await.unwrap();

    tracing::info!("Creating second rune...");
    let rune_id_2 = rune_manager.etch_new_rune(
        Some("TESTRUNETWO".to_string()),
        500,
        500_000
    ).await.unwrap();

    tracing::info!("Creating third rune...");
    let rune_id_3 = rune_manager.etch_new_rune(
        None,
        2000,
        2_000_000
    ).await.unwrap();

    let all_runes = rune_manager.get_managed_runes();
    assert_eq!(all_runes.len(), 3, "Should have 3 managed runes");

    let rune_ids = vec![rune_id_1, rune_id_2, rune_id_3];

    let mut user_wallet = UserWallet::new(
        bitcoin_client.clone(),
        rune_ids.clone()
    ).await.unwrap();

    tracing::info!("Minting rune 1...");
    rune_manager.mint_rune(rune_id_1, user_wallet.get_address()).await.unwrap();

    tracing::info!("Minting rune 2...");
    rune_manager.mint_rune(rune_id_2, user_wallet.get_address()).await.unwrap();

    tracing::info!("Minting rune 3...");
    rune_manager.mint_rune(rune_id_3, user_wallet.get_address()).await.unwrap();

    user_wallet.unite_unspent_utxos().await.unwrap();

    let all_balances = user_wallet.get_all_balances().await.unwrap();
    tracing::info!("All balances: {:?}", all_balances);
    println!("======================================={}", all_balances.len());
    assert!(all_balances.len() >= 3, "Should have at least 3 runes");

    for rune_id in &rune_ids {
        let balance = user_wallet.get_rune_balance(rune_id).await.unwrap();
        assert!(balance > 0, "Balance for rune {:?} should be greater than 0", rune_id);
        tracing::info!("Rune {:?} balance: {}", rune_id, balance);
    }

    let dummy_addresses: Vec<_> = (0..3).map(|_| create_credentials().0).collect();

    tracing::info!("Transferring rune 1...");
    user_wallet.transfer_runes(rune_id_1, 1000, dummy_addresses[0].clone()).await.unwrap();

    tracing::info!("Transferring rune 2...");
    user_wallet.transfer_runes(rune_id_2, 500, dummy_addresses[1].clone()).await.unwrap();

    tracing::info!("Transferring rune 3...");
    user_wallet.transfer_runes(rune_id_3, 2000, dummy_addresses[2].clone()).await.unwrap();

    for (i, (rune_id, dummy_addr)) in rune_ids.iter().zip(dummy_addresses.iter()).enumerate() {
        let address_data = bitcoin_client.get_address_data(dummy_addr.clone()).await.unwrap();
        tracing::info!("Dummy address {} data: {:?}", i + 1, address_data);

        let output = address_data.outputs[0].clone();
        assert_eq!(
            output.runes[0].rune_id.to_string(),
            rune_id.to_string(),
            "Rune ID mismatch for rune {}", i + 1
        );

        let expected_amounts = [1000u64, 500, 2000];
        assert_eq!(
            output.runes[0].amount as u64,
            expected_amounts[i],
            "Amount mismatch for rune {}", i + 1
        );
    }

    tracing::info!("All runes transferred successfully!");
}

#[tokio::test]
async fn test_rune_info_retrieval() {
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

    let rune_name = "INFORUNE".to_string();
    let cap = 777u128;
    let amount = 777_777u128;

    let rune_id = rune_manager.etch_new_rune(
        Some(rune_name.clone()),
        cap,
        amount
    ).await.unwrap();

    let rune_info = rune_manager.get_rune_info(&rune_id)
        .expect("Rune info should exist");

    assert_eq!(rune_info.name, rune_name);
    assert_eq!(rune_info.cap, cap);
    assert_eq!(rune_info.amount_per_mint, amount);
    assert_eq!(rune_info.rune_id, rune_id);

    tracing::info!("Rune info retrieved correctly: {:?}", rune_info);
}