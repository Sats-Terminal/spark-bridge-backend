use tests::gateway_client::*;

#[tokio::test]
async fn test_spark() {
    let gateway_client = GatewayClient::new(GatewayConfig {
        address: "http://localhost:8060".parse().unwrap(),
    });

    let get_runes_deposit_address_request = GetRunesDepositAddressRequest {
        user_public_key: "020202020202020202020202020202020202020202020202020202020202020202".to_string(),
        rune_id: "101:2".to_string(),
        amount: 1_000_000,
    };

    let get_runes_deposit_address_response = gateway_client
        .get_runes_deposit_address(get_runes_deposit_address_request)
        .await
        .unwrap();
    println!(
        "get_runes_deposit_address_response: {:?}",
        get_runes_deposit_address_response
    );

    let test_spark_request = TestSparkRequest {
        btc_address: get_runes_deposit_address_response.address,
    };

    let test_spark_response = gateway_client.test_spark(test_spark_request).await.unwrap();
    println!("test_spark_response: {:?}", test_spark_response);
}
