use breez_sdk_spark::{
    BreezSdk, Config, Network, PrepareSendPaymentResponse, SdkBuilder, Seed, SendPaymentMethod, SendPaymentRequest,
    SqliteStorage,
};
use std::{path::Path, sync::Arc};

use crate::error::SparkClientError;

pub struct SparkConfig {
    pub mnemonic: String,
    pub sync_interval: u32,
    pub sqlite_storage_path: String,
}

pub struct SparkBreezClient {
    sdk: BreezSdk,
}

impl SparkBreezClient {
    pub async fn new(spark_config: SparkConfig) -> Result<Self, SparkClientError> {
        let sdk_config = Config {
            api_key: None,
            network: Network::Regtest,
            sync_interval_secs: spark_config.sync_interval,
            max_deposit_claim_fee: None,
            lnurl_domain: None,
            prefer_spark_over_lightning: false,
            external_input_parsers: None,
            use_default_external_input_parsers: true,
        };

        let mnemonic = Seed::Mnemonic {
            mnemonic: spark_config.mnemonic,
            passphrase: None,
        };

        let storage_path = Path::new(&spark_config.sqlite_storage_path);
        let storage = SqliteStorage::new(storage_path)?;

        let sdk = SdkBuilder::new(sdk_config, mnemonic, Arc::new(storage)).build().await?;

        Ok(SparkBreezClient { sdk })
    }

    pub async fn transfer_spark_native(&self, destination: &str, amount: u128) -> Result<String, SparkClientError> {
        let resp = self
            .sdk
            .send_payment(SendPaymentRequest {
                prepare_response: PrepareSendPaymentResponse {
                    payment_method: SendPaymentMethod::SparkAddress {
                        address: destination.to_string(),
                        fee: 0,
                        token_identifier: None,
                    },
                    amount,
                    token_identifier: None,
                },
                options: None,
            })
            .await?;

        Ok(resp.payment.id)
    }
}
