use bitcoin::{Transaction, Txid};
use gateway_rune_transfer::errors::RuneTransferError;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct MockBitcoinClient {
    pub broadcasted_transactions: Arc<Mutex<Vec<Transaction>>>,
    pub transactions: Arc<Mutex<HashMap<Txid, Transaction>>>,
    pub should_fail_broadcast: Arc<Mutex<bool>>,
}

impl MockBitcoinClient {
    pub fn new() -> Self {
        Self {
            broadcasted_transactions: Arc::new(Mutex::new(Vec::new())),
            transactions: Arc::new(Mutex::new(HashMap::new())),
            should_fail_broadcast: Arc::new(Mutex::new(false)),
        }
    }

    pub async fn set_should_fail_broadcast(&self, fail: bool) {
        *self.should_fail_broadcast.lock().await = fail;
    }

    pub async fn get_broadcasted_transactions(&self) -> Vec<Transaction> {
        self.broadcasted_transactions.lock().await.clone()
    }

    pub async fn broadcast_count(&self) -> usize {
        self.broadcasted_transactions.lock().await.len()
    }

    pub async fn add_transaction(&self, tx: Transaction) {
        let txid = tx.compute_txid();
        self.transactions.lock().await.insert(txid, tx);
    }

    pub async fn broadcast_transaction(&self, tx: Transaction) -> Result<Txid, RuneTransferError> {
        if *self.should_fail_broadcast.lock().await {
            return Err(RuneTransferError::InvalidData(
                "Mock broadcast failure".to_string(),
            ));
        }

        let txid = tx.compute_txid();

        self.broadcasted_transactions.lock().await.push(tx.clone());
        self.transactions.lock().await.insert(txid, tx);

        Ok(txid)
    }

    pub async fn get_transaction(&self, txid: &Txid) -> Option<Transaction> {
        self.transactions.lock().await.get(txid).cloned()
    }

    pub async fn was_broadcasted(&self, txid: &Txid) -> bool {
        self.broadcasted_transactions
            .lock()
            .await
            .iter()
            .any(|tx| tx.compute_txid() == *txid)
    }
}

impl Default for MockBitcoinClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::{Amount, ScriptBuf};

    fn create_dummy_transaction() -> Transaction {
        Transaction {
            version: bitcoin::transaction::Version::TWO,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vec![],
            output: vec![bitcoin::TxOut {
                value: Amount::from_sat(1000),
                script_pubkey: ScriptBuf::new(),
            }],
        }
    }

    #[tokio::test]
    async fn test_mock_bitcoin_client_broadcasts() {
        let client = MockBitcoinClient::new();
        let tx = create_dummy_transaction();
        let txid = tx.compute_txid();

        let result = client.broadcast_transaction(tx.clone()).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), txid);

        assert_eq!(client.broadcast_count().await, 1);
        assert!(client.was_broadcasted(&txid).await);
    }

    #[tokio::test]
    async fn test_mock_bitcoin_client_can_fail() {
        let client = MockBitcoinClient::new();
        client.set_should_fail_broadcast(true).await;

        let tx = create_dummy_transaction();
        let result = client.broadcast_transaction(tx).await;

        assert!(result.is_err());
        assert_eq!(client.broadcast_count().await, 0);
    }

    #[tokio::test]
    async fn test_mock_bitcoin_client_stores_transactions() {
        let client = MockBitcoinClient::new();
        let tx = create_dummy_transaction();
        let txid = tx.compute_txid();

        client.broadcast_transaction(tx.clone()).await.unwrap();

        let retrieved = client.get_transaction(&txid).await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().compute_txid(), txid);
    }
}