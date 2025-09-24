use tests::gateway_client::*;

#[tokio::test]
async fn test_spark() {
    let gateway_client = GatewayClient::new(GatewayConfig {
        address: "http://localhost:8060".parse().unwrap(),
    });

    let get_runes_deposit_address_request = GetRunesDepositAddressRequest {
        user_public_key: "0324f3e4fdd2d2c3f95d26d281868bbe4d68c6c7b573dbf57bf0d3454fd77b88c0".to_string(),
        rune_id: "101:3".to_string(),
        amount: 1_000_000,
    };

    let get_runes_deposit_address_response = gateway_client
        .get_runes_deposit_address(get_runes_deposit_address_request).await.unwrap();
    println!("get_runes_deposit_address_response: {:?}", get_runes_deposit_address_response);

    let test_spark_request = TestSparkRequest {
        btc_address: get_runes_deposit_address_response.address,
    };

    let test_spark_response = gateway_client.test_spark(test_spark_request).await.unwrap();
    println!("test_spark_response: {:?}", test_spark_response);
}
