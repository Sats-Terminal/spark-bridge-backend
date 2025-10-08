use frost::types::MusigId;
use gateway_spark_service::errors::SparkServiceError;
use gateway_spark_service::types::SparkTransactionType;
use spark_address::Network; 
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct MockSparkService {
    pub sent_transactions: Arc<Mutex<Vec<MockSparkTransaction>>>,
    pub should_fail: Arc<Mutex<bool>>,
}

#[derive(Debug, Clone)]
pub struct MockSparkTransaction {
    pub musig_id: MusigId,
    pub invoice: Option<String>,
    pub token_identifier: String,
    pub tx_type: SparkTransactionType,
    pub network: Network, 
}

impl MockSparkService {
    pub fn new() -> Self {
        Self {
            sent_transactions: Arc::new(Mutex::new(Vec::new())),
            should_fail: Arc::new(Mutex::new(false)),
        }
    }

    pub async fn set_should_fail(&self, fail: bool) {
        *self.should_fail.lock().await = fail;
    }

    pub async fn get_sent_transactions(&self) -> Vec<MockSparkTransaction> {
        self.sent_transactions.lock().await.clone()
    }

    pub async fn has_create_transaction(&self) -> bool {
        self.sent_transactions
            .lock()
            .await
            .iter()
            .any(|tx| matches!(tx.tx_type, SparkTransactionType::Create { .. }))
    }

    pub async fn has_mint_transaction(&self) -> bool {
        self.sent_transactions
            .lock()
            .await
            .iter()
            .any(|tx| matches!(tx.tx_type, SparkTransactionType::Mint { .. }))
    }

    pub async fn send_spark_transaction(
        &self,
        musig_id: MusigId,
        invoice: Option<String>,
        token_identifier: String,
        tx_type: SparkTransactionType,
        network: Network, // Используем Network из spark_address
    ) -> Result<(), SparkServiceError> {
        if *self.should_fail.lock().await {
            return Err(SparkServiceError::SparkClientError(
                "Mock failure".to_string(),
            ));
        }

        let tx = MockSparkTransaction {
            musig_id,
            invoice,
            token_identifier,
            tx_type,
            network,
        };

        self.sent_transactions.lock().await.push(tx);

        Ok(())
    }
}

impl Default for MockSparkService {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone)]
pub struct MockSparkClient {
    pub connected: Arc<Mutex<bool>>,
}

impl MockSparkClient {
    pub fn new() -> Self {
        Self {
            connected: Arc::new(Mutex::new(true)),
        }
    }

    pub async fn is_connected(&self) -> bool {
        *self.connected.lock().await
    }

    pub async fn set_connected(&self, connected: bool) {
        *self.connected.lock().await = connected;
    }
}

impl Default for MockSparkClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::secp256k1::PublicKey;
    use spark_address::Network;
    use std::str::FromStr;
    
    fn create_test_public_key() -> PublicKey {
        PublicKey::from_str(
            "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"
        ).unwrap()
    }

    #[tokio::test]
    async fn test_mock_spark_service_records_transactions() {
        let service = MockSparkService::new();
        let test_pubkey = create_test_public_key();

        service
            .send_spark_transaction(
                MusigId::Issuer {
                    issuer_public_key: test_pubkey,
                    rune_id: "840000:1".to_string(),
                },
                None,
                "test_token".to_string(),
                SparkTransactionType::Create {
                    token_name: "Test Token".to_string(),
                    token_ticker: "TST".to_string(),
                },
                Network::Regtest,
            )
            .await
            .unwrap();

        assert_eq!(service.get_sent_transactions().await.len(), 1);
        assert!(service.has_create_transaction().await);
    }

    #[tokio::test]
    async fn test_mock_spark_service_can_fail() {
        let service = MockSparkService::new();
        let test_pubkey = create_test_public_key();

        service.set_should_fail(true).await;

        let result = service
            .send_spark_transaction(
                MusigId::Issuer {
                    issuer_public_key: test_pubkey,
                    rune_id: "840000:1".to_string(),
                },
                None,
                "test".to_string(),
                SparkTransactionType::Create {
                    token_name: "Test".to_string(),
                    token_ticker: "TST".to_string(),
                },
                Network::Regtest,
            )
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_mock_spark_service_tracks_mint_transactions() {
        let service = MockSparkService::new();
        let test_pubkey = create_test_public_key();

        service
            .send_spark_transaction(
                MusigId::User {
                    user_public_key: test_pubkey,
                    rune_id: "840000:1".to_string(),
                },
                Some("lnbc1000n1...".to_string()),
                "test_token".to_string(),
                SparkTransactionType::Mint {
                    receiver_spark_address: "sprt1pgssy7d7vel0nh9m4326qc54e6rskpczn07dktww9rv4nu5ptvt0s9ucd5rgc0".to_string(),
                    token_amount: 1000,
                },
                Network::Regtest,
            )
            .await
            .unwrap();

        assert!(service.has_mint_transaction().await);
        assert_eq!(service.get_sent_transactions().await.len(), 1);
    }

    #[tokio::test]
    async fn test_mock_spark_service_different_networks() {
        let service = MockSparkService::new();
        let test_pubkey = create_test_public_key();

        service
            .send_spark_transaction(
                MusigId::Issuer {
                    issuer_public_key: test_pubkey,
                    rune_id: "840000:1".to_string(),
                },
                None,
                "test_token".to_string(),
                SparkTransactionType::Create {
                    token_name: "Test Token".to_string(),
                    token_ticker: "TST".to_string(),
                },
                Network::Testnet,
            )
            .await
            .unwrap();

        service
            .send_spark_transaction(
                MusigId::User {
                    user_public_key: test_pubkey,
                    rune_id: "840000:2".to_string(),
                },
                None,
                "main_token".to_string(),
                SparkTransactionType::Mint {
                    receiver_spark_address: "sp1pgssy7d7vel0nh9m4326qc54e6rskpczn07dktww9rv4nu5ptvt0s9ucez8h3s".to_string(),
                    token_amount: 5000,
                },
                Network::Mainnet,
            )
            .await
            .unwrap();

        let transactions = service.get_sent_transactions().await;
        assert_eq!(transactions.len(), 2);
        assert_eq!(transactions[0].network, Network::Testnet);
        assert_eq!(transactions[1].network, Network::Mainnet);
    }
}