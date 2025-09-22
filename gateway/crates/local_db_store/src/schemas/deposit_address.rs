use crate::storage::LocalDbStorage;
use async_trait::async_trait;
use bitcoin::Address;
use bitcoin::Txid;
use frost::types::MusigId;
use frost::types::Nonce;
use persistent_storage::error::DbError;
use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use std::collections::HashMap;
use std::fmt::Debug;
use std::str::FromStr;

#[derive(Debug, Clone, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DepositStatus {
    Created,
    WaitingForConfirmation,
    Confirmed,
    Failed(TxRejectReason),
}

#[derive(Deserialize, Serialize, Debug, Clone, Eq, PartialEq)]
pub enum TxRejectReason {
    NoRunesInOuts,
    NoFeesPayed,
    TooFewSatoshiPaidAsFee { got: u64, at_least_expected: u64 },
    NoExpectedVOutInOutputs { got: u64, expected: u64 },
    NoExpectedTOutWithRunes,
    NoExpectedTOutWithRunesAmount { amount: u64 },
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
                DepositStatus::Failed(_) => return false,
                DepositStatus::WaitingForConfirmation => return false,
                DepositStatus::Created => return false,
            }
        }
        true
    }

    pub fn empty() -> Self {
        VerifiersResponses { responses: HashMap::new() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct DepositAddrInfo {
    pub musig_id: MusigId,
    pub nonce: Nonce,
    pub deposit_address: String,
    pub bridge_address: Option<String>,
    pub is_btc: bool,
    pub amount: u64,
    pub confirmation_status: VerifiersResponses,
}

#[async_trait]
pub trait DepositAddressStorage: Send + Sync + Debug {
    async fn get_deposit_addr_info(&self, musig_id: &MusigId, tweak: Nonce) -> Result<Option<DepositAddrInfo>, DbError>;
    async fn set_deposit_addr_info(&self, deposit_addr_info: DepositAddrInfo) -> Result<(), DbError>;
    async fn set_confirmation_status_by_deposit_address(&self, address: String, confirmation_status: VerifiersResponses) -> Result<(), DbError>;
    async fn get_row_by_deposit_address(&self, address: String) -> Result<Option<DepositAddrInfo>, DbError>;
    async fn update_confirmation_status_by_deposit_address(&self, deposit_address: String, verifier_id: u16, verifier_response: DepositStatus) -> Result<(), DbError>;
    async fn update_bridge_address_by_deposit_address(&self, deposit_address: String, bridge_address: String) -> Result<(), DbError>;
}

#[async_trait]
impl DepositAddressStorage for LocalDbStorage {
    async fn get_deposit_addr_info(
        &self,
        musig_id: &MusigId,
        tweak: Nonce,
    ) -> Result<Option<DepositAddrInfo>, DbError> {
        let public_key = musig_id.get_public_key();
        let rune_id = musig_id.get_rune_id();

        let result: Option<(String, Option<String>, bool, i64, Json<VerifiersResponses>)> = sqlx::query_as(
            "SELECT deposit_address, bridge_address, is_btc, amount, confirmation_status
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
            Some((deposit_address, bridge_address, is_btc, amount, confirmation_status)) => {
                Ok(Some(DepositAddrInfo {
                    musig_id: musig_id.clone(),
                    nonce: tweak,
                    deposit_address,
                    bridge_address,
                    is_btc,
                    amount: amount as u64,
                    confirmation_status: confirmation_status.0,
                }))
            }
            None => Ok(None),
        }

    }

    async fn set_deposit_addr_info(&self, deposit_addr_info: DepositAddrInfo) -> Result<(), DbError> {

        let _ = sqlx::query(
            "INSERT INTO gateway.deposit_address (public_key, rune_id, nonce_tweak, deposit_address, bridge_address, is_btc, amount, confirmation_status)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (public_key, rune_id, nonce_tweak) DO UPDATE SET confirmation_status = $7",
        )
            .bind(deposit_addr_info.musig_id.get_public_key().to_string())
            .bind(deposit_addr_info.musig_id.get_rune_id())
            .bind(deposit_addr_info.nonce)
            .bind(deposit_addr_info.deposit_address)
            .bind(deposit_addr_info.bridge_address)
            .bind(deposit_addr_info.is_btc)
            .bind(deposit_addr_info.amount as i64)
            .bind(Json(deposit_addr_info.confirmation_status))
            .execute(&self.get_conn().await?)
            .await
            .map_err(|e| DbError::BadRequest(e.to_string()))?;

        Ok(())
    }

    async fn set_confirmation_status_by_deposit_address(&self, address: String, confirmation_status: VerifiersResponses) -> Result<(), DbError> {
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

    async fn get_row_by_deposit_address(&self, deposit_address: String) -> Result<Option<DepositAddrInfo>, DbError> {
        let result: Option<(String, String, Nonce, String, Option<String>, bool, i64, Json<VerifiersResponses>)> = sqlx::query_as(
            "SELECT public_key, rune_id, nonce_tweak, deposit_address, bridge_address, is_btc, amount, confirmation_status
            FROM gateway.deposit_address WHERE deposit_address = $1",
        )
        .bind(deposit_address)
        .fetch_optional(&self.get_conn().await?)
        .await
        .map_err(|e| DbError::BadRequest(e.to_string()))?;

        match result {
            Some((public_key, rune_id, nonce_tweak, deposit_address, bridge_address, is_btc, amount, confirmation_status)) => {
                let musig_id = MusigId::User {
                    rune_id,
                    user_public_key: bitcoin::secp256k1::PublicKey::from_str(&public_key).unwrap(),
                };
                let nonce = Nonce::from(nonce_tweak);
                let deposit_addr_info = DepositAddrInfo {
                    musig_id,
                    nonce,
                    deposit_address,
                    bridge_address,
                    is_btc,
                    amount: amount as u64,
                    confirmation_status: confirmation_status.0,
                };
                Ok(Some(deposit_addr_info))
            }
            None => Ok(None),
        }
    }

    async fn update_confirmation_status_by_deposit_address(&self, deposit_address: String, verifier_id: u16, verifier_response: DepositStatus) -> Result<(), DbError> {
        let mut tx = self.get_conn().await?.begin().await?;

        let response: Option<(Json<VerifiersResponses>, )> = sqlx::query_as(
            "SELECT confirmation_status 
            FROM gateway.deposit_address 
            WHERE deposit_address = $1
            FOR UPDATE",
        )
            .bind(deposit_address.clone())
            .fetch_optional(&mut *tx)
            .await?;
        
        let mut confirmation_status = match response {
            Some((confirmation_status, )) => confirmation_status.0,
            None => return Err(DbError::NotFound(format!("Confirmation status not found for address: {}", deposit_address))),
        };

        confirmation_status.responses.insert(verifier_id, verifier_response);
        
        let _ = sqlx::query(
            "UPDATE gateway.deposit_address 
            SET confirmation_status = $1 
            WHERE address = $2",
        )
            .bind(Json(confirmation_status))
            .bind(deposit_address)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;

        Ok(())
    }

    async fn update_bridge_address_by_deposit_address(&self, deposit_address: String, bridge_address: String) -> Result<(), DbError> {
        let _ = sqlx::query(
            "UPDATE gateway.deposit_address 
            SET bridge_address = $1 
            WHERE deposit_address = $2",
        )
            .bind(bridge_address)
            .bind(deposit_address)
            .execute(&self.get_conn().await?)
            .await?;

        Ok(())
    }
}
