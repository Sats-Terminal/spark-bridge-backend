use crate::storage::LocalDbStorage;
use async_trait::async_trait;
use frost::types::MusigId;
use frost::types::Nonce;
use persistent_storage::error::DbError;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use sqlx::types::Json;
use bitcoin::Txid;
use std::collections::HashMap;
use std::str::FromStr;
use bitcoin::Address;

#[derive(Debug, Clone, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DepositStatus {
    Created,
    WaitingForConfirmation,
    Confirmed,
    Failed,
}

#[derive(Debug, Clone, Deserialize, Serialize, Eq, PartialEq)]
pub struct VerifiersResponses {
    pub responses: HashMap<u16, DepositStatus>,
}

impl VerifiersResponses {
    pub fn new(status: DepositStatus, ids: Vec<u16>) -> Self {
        let mut responses = HashMap::new();
        for id in ids {
            responses.insert(id, status.clone());
        }
        VerifiersResponses { responses }
    }

    pub fn check_all_verifiers_confirmed(&self) -> bool {
        for response in self.responses.values() {
            match response {
                DepositStatus::Confirmed => continue,
                DepositStatus::Failed => return false,
                DepositStatus::WaitingForConfirmation => return false,
                DepositStatus::Created => return false,
            }
        }
        true
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct DepositAddrInfo {
    pub address: Option<String>,
    pub is_btc: bool,
    pub amount: u64,
    pub confirmation_status: VerifiersResponses,
}

#[async_trait]
pub trait DepositAddressStorage: Send + Sync + Debug {
    async fn get_deposit_addr_info(&self, musig_id: &MusigId, tweak: Nonce) -> Result<Option<DepositAddrInfo>, DbError>;
    async fn set_deposit_addr_info(&self, musig_id: &MusigId, tweak: Nonce, deposit_addr_info: DepositAddrInfo) -> Result<(), DbError>;
    async fn set_confirmation_status_by_address(&self, address: String, confirmation_status: VerifiersResponses) -> Result<(), DbError>;
    async fn get_row_by_address(&self, address: String) -> Result<Option<(MusigId, Nonce, DepositAddrInfo)>, DbError>;
    async fn get_confirmation_status_by_address(&self, address: String) -> Result<Option<VerifiersResponses>, DbError>;
    async fn set_txid(&self, address: String, txid: Txid) -> Result<(), DbError>;
    async fn get_confirmation_status_by_txid(&self, txid: Txid) -> Result<Option<VerifiersResponses>, DbError>;
    async fn update_confirmation_status_by_txid(&self, txid: Txid, verifier_id: u16, confirmation_status: DepositStatus) -> Result<(), DbError>;
    async fn get_address_by_txid(&self, txid: Txid) -> Result<Option<Address>, DbError>;
}

#[async_trait]
impl DepositAddressStorage for LocalDbStorage {
    async fn get_deposit_addr_info(&self, musig_id: &MusigId, tweak: Nonce) -> Result<Option<DepositAddrInfo>, DbError> {
        let public_key = musig_id.get_public_key();
        let rune_id = musig_id.get_rune_id();

        let result: Option<(Option<String>, bool, i64, Option<String>, Json<VerifiersResponses>)> = sqlx::query_as(
            "SELECT address, is_btc, amount, txid, confirmation_status
            FROM gateway.deposit_address
            WHERE public_key = $1 AND rune_id = $2 AND nonce_tweak = $3",
        )
            .bind(public_key.to_string())
            .bind(rune_id)
            .bind(tweak)
            .fetch_optional(&self.get_conn().await?)
            .await
            .map_err(|e| DbError::BadRequest(e.to_string()))?;
        
        match result {
            Some((address, is_btc, amount, txid_str, confirmation_status)) => {
                let txid = match txid_str {
                    Some(s) => Some(Txid::from_str(&s).map_err(|e| DbError::DecodeError(format!("Failed to decode txid: {}", e)))?),
                    None => None,
                };
                Ok(Some(DepositAddrInfo {
                    address,
                    is_btc,
                    amount: amount as u64,
                    txid,
                    confirmation_status: confirmation_status.0,
                }))
            }
            None => Ok(None),
        }
        
    }

    async fn set_deposit_addr_info(&self, musig_id: &MusigId, tweak: Nonce, deposit_addr_info: DepositAddrInfo) -> Result<(), DbError> {
        let confirmation_status = Json(deposit_addr_info.confirmation_status);
        let public_key = musig_id.get_public_key();
        let rune_id = musig_id.get_rune_id();

        let _ = sqlx::query(
            "INSERT INTO gateway.deposit_address (public_key, rune_id, nonce_tweak, address, is_btc, amount, txid, confirmation_status)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            ON CONFLICT (public_key, rune_id, nonce_tweak) DO UPDATE SET confirmation_status = $8",
        )
            .bind(public_key.to_string())
            .bind(rune_id)
            .bind(tweak)
            .bind(deposit_addr_info.address)
            .bind(deposit_addr_info.is_btc)
            .bind(deposit_addr_info.amount as i64)
            .bind(deposit_addr_info.txid.map(|txid| txid.to_string()))
            .bind(confirmation_status)
            .execute(&self.get_conn().await?)
            .await
            .map_err(|e| DbError::BadRequest(e.to_string()))?;

        Ok(())
    }

    async fn set_confirmation_status_by_address(&self, address: String, confirmation_status: VerifiersResponses) -> Result<(), DbError> {
        let _ = sqlx::query(
            "UPDATE gateway.deposit_address SET confirmation_status = $1 WHERE address = $2",
        )
            .bind(Json(confirmation_status))
            .bind(address)
            .execute(&self.get_conn().await?)
            .await
            .map_err(|e| DbError::BadRequest(e.to_string()))?;

        Ok(())
    }

    async fn get_row_by_address(&self, address: String) -> Result<Option<(MusigId, Nonce, DepositAddrInfo)>, DbError> {
        let result: Option<(String, String, Nonce, Option<String>, bool, i64, Option<String>, Json<VerifiersResponses>)> = sqlx::query_as(
            "SELECT public_key, rune_id, nonce_tweak, address, is_btc, amount, txid, confirmation_status
            FROM gateway.deposit_address WHERE address = $1",
        )
            .bind(address)
            .fetch_optional(&self.get_conn().await?)
            .await
            .map_err(|e| DbError::BadRequest(e.to_string()))?;

        match result {
            Some((public_key, rune_id, nonce_tweak, address, is_btc, amount, txid_str, confirmation_status)) => {
                let musig_id = MusigId::User {
                    rune_id,
                    user_public_key: bitcoin::secp256k1::PublicKey::from_str(&public_key).unwrap(),
                };
                let nonce = Nonce::from(nonce_tweak);
                let txid = match txid_str {
                    Some(s) => Some(Txid::from_str(&s).map_err(|e| DbError::DecodeError(format!("Failed to decode txid: {}", e)))?),
                    None => None,
                };
                let deposit_addr_info = DepositAddrInfo {
                    address,
                    is_btc,
                    amount: amount as u64,
                    txid,
                    confirmation_status: confirmation_status.0,
                };
                Ok(Some((musig_id, nonce, deposit_addr_info)))
            }
            None => Ok(None),
        }
    }

    async fn get_confirmation_status_by_address(&self, address: String) -> Result<Option<VerifiersResponses>, DbError> {
        let result: Option<(Json<VerifiersResponses>, )> = sqlx::query_as(
            "SELECT confirmation_status
            FROM gateway.deposit_address WHERE address = $1",
        )
            .bind(address)
            .fetch_optional(&self.get_conn().await?)
            .await
            .map_err(|e| DbError::BadRequest(e.to_string()))?;

        Ok(result.map(|deposit_status_info| deposit_status_info.0.0))
    }

    async fn set_txid(&self, address: String, txid: Txid) -> Result<(), DbError> {
        let _ = sqlx::query(
            "UPDATE gateway.deposit_address SET txid = $1 WHERE address = $2",
        )
            .bind(txid.to_string())
            .bind(address)
            .execute(&self.get_conn().await?)
            .await
            .map_err(|e| DbError::BadRequest(e.to_string()))?;

        Ok(())
    }

    async fn get_confirmation_status_by_txid(&self, txid: Txid) -> Result<Option<VerifiersResponses>, DbError> {
        let result: Option<(Json<VerifiersResponses>, )> = sqlx::query_as(
            "SELECT confirmation_status
            FROM gateway.deposit_address WHERE txid = $1",
        )
            .bind(txid.to_string())
            .fetch_optional(&self.get_conn().await?)
            .await
            .map_err(|e| DbError::BadRequest(e.to_string()))?;

        Ok(result.map(|deposit_status_info| deposit_status_info.0.0))
    }

    async fn update_confirmation_status_by_txid(&self, txid: Txid, verifier_id: u16, confirmation_status: DepositStatus) -> Result<(), DbError> {
        let mut tx = self.get_conn().await?.begin().await?;

        let result: Option<(Json<VerifiersResponses>,)> = sqlx::query_as(
            "SELECT confirmation_status
            FROM gateway.deposit_address WHERE txid = $1",
        )
            .bind(txid.to_string())
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| DbError::BadRequest(e.to_string()))?;

        let mut status = match result {
            Some((status, )) => status.0,
            None => return Err(DbError::BadRequest("Confirmation status not found".to_string())),
        };

        status.responses.insert(verifier_id, confirmation_status);

        let _ = sqlx::query(
            "UPDATE gateway.deposit_address SET confirmation_status = $1 WHERE txid = $2",
        )
            .bind(Json(status))
            .bind(txid.to_string())
            .execute(&mut *tx)
            .await
            .map_err(|e| DbError::BadRequest(format!("Failed to update confirmation status: {}", e)))?;

        let _ = tx.commit().await.map_err(|e| DbError::BadRequest(format!("Failed to commit transaction: {}", e)))?;

        Ok(())
    }

    async fn get_address_by_txid(&self, txid: Txid) -> Result<Option<Address>, DbError> {
        let result: Option<(String, )> = sqlx::query_as(
            "SELECT address
            FROM gateway.deposit_address WHERE txid = $1",
        )
            .bind(txid.to_string())
            .fetch_optional(&self.get_conn().await?)
            .await
            .map_err(|e| DbError::BadRequest(e.to_string()))?;

        match result {
            Some((address, )) => {
                let address = Address::from_str(&address).map_err(|e| DbError::DecodeError(format!("Failed to decode address: {}", e)))?;
                Ok(Some(address.assume_checked()))
            }
            None => Ok(None),
        }
    }
}
