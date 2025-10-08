use async_trait::async_trait;
use bitcoin::{Address, OutPoint, Txid};
use frost::types::{MusigId, TweakBytes};
use persistent_storage::error::DbError;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use gateway_local_db_store::schemas::deposit_address::{
    DepositAddressStorage, DepositAddrInfo, DepositStatus, InnerAddress, VerifiersResponses,
};
use gateway_local_db_store::schemas::paying_utxo::PayingUtxoStorage;
use gateway_local_db_store::schemas::utxo_storage::{UtxoStorage, Utxo, UtxoStatus};
use gateway_rune_transfer::transfer::PayingTransferInput;
use persistent_storage::init::StorageHealthcheck;

#[derive(Default, Clone)]
pub struct MockStorage {
    deposit_addresses: Arc<RwLock<HashMap<String, DepositAddrInfo>>>,
    paying_utxos: Arc<RwLock<HashMap<String, PayingTransferInput>>>,
    utxos: Arc<RwLock<HashMap<String, Utxo>>>,
}

impl MockStorage {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn insert_test_musig_id(
        &self,
        musig_id: MusigId,
        tweak: TweakBytes,
        info: DepositAddrInfo,
    ) {
        let key = format!("{}-{:?}", musig_id.get_rune_id(), tweak);
        self.deposit_addresses.write().await.insert(key, info);
    }

    pub async fn insert_test_deposit_address(&self, info: DepositAddrInfo) {
        let key = info.deposit_address.to_string();
        self.deposit_addresses.write().await.insert(key, info);
    }

    pub async fn insert_test_paying_utxo(&self, paying: PayingTransferInput) {
        self.paying_utxos
            .write()
            .await
            .insert(paying.btc_exit_address.to_string(), paying);
    }

    pub async fn insert_test_utxo(&self, utxo: Utxo) {
        self.utxos
            .write()
            .await
            .insert(utxo.out_point.to_string(), utxo);
    }

    pub async fn deposit_addresses_count(&self) -> usize {
        self.deposit_addresses.read().await.len()
    }

    pub async fn utxos_count(&self) -> usize {
        self.utxos.read().await.len()
    }
}

#[async_trait]
impl DepositAddressStorage for MockStorage {
    async fn get_deposit_addr_info(
        &self,
        musig_id: &MusigId,
        tweak: TweakBytes,
    ) -> Result<Option<DepositAddrInfo>, DbError> {
        let key = format!("{}-{:?}", musig_id.get_rune_id(), tweak);
        Ok(self.deposit_addresses.read().await.get(&key).cloned())
    }

    async fn set_deposit_addr_info(&self, deposit_addr_info: DepositAddrInfo) -> Result<(), DbError> {
        let key = deposit_addr_info.deposit_address.to_string();
        self.deposit_addresses.write().await.insert(key, deposit_addr_info);
        Ok(())
    }

    async fn set_confirmation_status_by_deposit_address(
        &self,
        address: InnerAddress,
        confirmation_status: VerifiersResponses,
    ) -> Result<(), DbError> {
        let key = address.to_string();
        if let Some(mut info) = self.deposit_addresses.write().await.get_mut(&key) {
            info.confirmation_status = confirmation_status;
            Ok(())
        } else {
            Err(DbError::NotFound("Address not found".to_string()))
        }
    }

    async fn get_row_by_deposit_address(
        &self,
        address: InnerAddress,
    ) -> Result<Option<DepositAddrInfo>, DbError> {
        let key = address.to_string();
        Ok(self.deposit_addresses.read().await.get(&key).cloned())
    }

    async fn update_confirmation_status_by_deposit_address(
        &self,
        address: InnerAddress,
        verifier_id: u16,
        verifier_response: DepositStatus,
    ) -> Result<(), DbError> {
        let key = address.to_string();
        if let Some(mut info) = self.deposit_addresses.write().await.get_mut(&key) {
            info.confirmation_status
                .responses
                .insert(verifier_id, verifier_response);
            Ok(())
        } else {
            Err(DbError::NotFound("Deposit address not found".to_string()))
        }
    }

    async fn update_bridge_address_by_deposit_address(
        &self,
        deposit_address: InnerAddress,
        bridge_address: InnerAddress,
    ) -> Result<(), DbError> {
        let key = deposit_address.to_string();
        if let Some(mut info) = self.deposit_addresses.write().await.get_mut(&key) {
            info.bridge_address = Some(bridge_address);
            Ok(())
        } else {
            Err(DbError::NotFound("Deposit address not found".to_string()))
        }
    }
}

#[async_trait]
impl StorageHealthcheck for MockStorage {
    async fn healthcheck(&self) -> Result<(), DbError> {
        Ok(())
    }
}

#[async_trait]
impl PayingUtxoStorage for MockStorage {
    async fn insert_paying_utxo(&self, paying_utxo: PayingTransferInput) -> Result<(), DbError> {
        self.paying_utxos
            .write()
            .await
            .insert(paying_utxo.btc_exit_address.to_string(), paying_utxo);
        Ok(())
    }

    async fn get_paying_utxo_by_btc_exit_address(
        &self,
        btc_exit_address: Address,
    ) -> Result<Option<PayingTransferInput>, DbError> {
        Ok(self
            .paying_utxos
            .read()
            .await
            .get(&btc_exit_address.to_string())
            .cloned())
    }
}

#[async_trait]
impl UtxoStorage for MockStorage {
    async fn insert_utxo(&self, utxo: Utxo) -> Result<Utxo, DbError> {
        self.utxos
            .write()
            .await
            .insert(utxo.out_point.to_string(), utxo.clone());
        Ok(utxo)
    }

    async fn update_status(&self, out_point: OutPoint, new_status: UtxoStatus) -> Result<(), DbError> {
        let key = out_point.to_string();
        if let Some(mut utxo) = self.utxos.write().await.get_mut(&key) {
            utxo.status = new_status;
            Ok(())
        } else {
            Err(DbError::NotFound(format!("UTXO {} not found", key)))
        }
    }

    async fn list_unspent(&self, rune_id: String) -> Result<Vec<Utxo>, DbError> {
        let utxos = self
            .utxos
            .read()
            .await
            .values()
            .filter(|u| u.rune_id == rune_id && matches!(u.status, UtxoStatus::Pending | UtxoStatus::Confirmed))
            .cloned()
            .collect();
        Ok(utxos)
    }

    async fn select_utxos_for_amount(&self, rune_id: String, target_amount: u64) -> Result<Vec<Utxo>, DbError> {
        let mut selected = Vec::new();
        let mut total = 0u64;

        for utxo in self
            .utxos
            .read()
            .await
            .values()
            .filter(|u| u.rune_id == rune_id && matches!(u.status, UtxoStatus::Pending | UtxoStatus::Confirmed))
        {
            if total < target_amount {
                total += utxo.rune_amount;
                selected.push(utxo.clone());
            } else {
                break;
            }
        }

        if total < target_amount {
            return Err(DbError::BadRequest("Not enough funds".into()));
        }

        let mut map = self.utxos.write().await;
        for u in &selected {
            if let Some(utxo) = map.get_mut(&u.out_point.to_string()) {
                utxo.status = UtxoStatus::Spent;
            }
        }

        Ok(selected)
    }

    async fn get_utxo(&self, out_point: OutPoint) -> Result<Option<Utxo>, DbError> {
        Ok(self.utxos.read().await.get(&out_point.to_string()).cloned())
    }

    async fn delete_utxo(&self, out_point: OutPoint) -> Result<(), DbError> {
        let removed = self.utxos.write().await.remove(&out_point.to_string());
        if removed.is_some() {
            Ok(())
        } else {
            Err(DbError::NotFound(format!("UTXO {} not found", out_point)))
        }
    }

    async fn update_sats_fee_amount(&self, out_point: OutPoint, sats_fee_amount: u64) -> Result<(), DbError> {
        let key = out_point.to_string();
        if let Some(mut utxo) = self.utxos.write().await.get_mut(&key) {
            utxo.sats_fee_amount = sats_fee_amount;
            Ok(())
        } else {
            Err(DbError::NotFound(format!("UTXO {} not found", key)))
        }
    }

    async fn get_utxo_by_btc_address(&self, btc_address: String) -> Result<Option<Utxo>, DbError> {
        let found = self
            .utxos
            .read()
            .await
            .values()
            .find(|u| u.btc_address.to_string() == btc_address)
            .cloned();
        Ok(found)
    }
}