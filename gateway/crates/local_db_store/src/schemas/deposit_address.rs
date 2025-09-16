use crate::storage::LocalDbStorage;
use async_trait::async_trait;
use frost::types::MusigId;
use frost::types::Nonce;
use persistent_storage::error::DbError;
use serde::{Deserialize, Serialize};
use sqlx::Connection;
use sqlx::types::Json;
use tracing::instrument;
use bitcoin::Txid;
use std::collections::HashMap;

type Nonce = [u8; 32];

#[derive(Debug, Clone, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DepositStatus {
    Created,
    WaitingForConfirmation,
    Confirmed,
    Failed,
}

#[derive(Debug, Clone, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct DepositStatusInfo {
    pub txid: Option<Txid>,
    pub status: DepositStatus,
    pub verifiers_responses: Option<HashMap<u16, bool>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct DepositAddrInfo {
    pub address: Option<String>,
    pub is_btc: bool,
    pub amount: u64,
    pub confirmation_status: DepositStatus,
}

#[async_trait]
pub trait DepositAddressStorage {
    async fn get_deposit_addr_info(&self, musig_id: &MusigId, tweak: Nonce) -> Result<Option<DepositAddrInfo>, DbError>;
    async fn set_deposit_addr_info(&self, musig_id: &MusigId, tweak: Nonce, deposit_addr_info: DepositAddrInfo) -> Result<(), DbError>;
    async fn get_confirmation_status(&self, musig_id: &MusigId, tweak: Nonce) -> Result<Option<DepositStatusInfo>, DbError>;
    async fn update_confirmation_status(&self, musig_id: &MusigId, tweak: Nonce, confirmation_status: DepositStatusInfo) -> Result<(), DbError>;
}

#[async_trait]
impl DepositAddressStorage for LocalDbStorage {
    async fn get_deposit_addr_info(&self, musig_id: &MusigId, tweak: Nonce) -> Result<Option<DepositAddrInfo>, DbError> {
        let public_key = musig_id.get_public_key();
        let rune_id = musig_id.get_rune_id();

        let result: Option<(Option<String>, bool, i64, Json<DepositStatus>)> = sqlx::query_as(
            "SELECT address, is_btc, amount, confirmation_status
            FROM verifier.deposit_address
            WHERE public_key = $1 AND rune_id = $2",
        )
            .bind(public_key.to_string())
            .bind(rune_id)
            .fetch_optional(&self.get_conn().await?)
            .await
            .map_err(|e| DbError::BadRequest(e.to_string()))?;
        
        Ok(result.map(|(address, is_btc, amount, confirmation_status)| DepositAddrInfo {
            address,
            is_btc,
            amount: amount as u64,
            confirmation_status: confirmation_status.0,
        }))
    }

    async fn set_deposit_addr_info(&self, musig_id: &MusigId, tweak: Nonce, deposit_addr_info: DepositAddrInfo) -> Result<(), DbError> {
        let confirmation_status = Json(deposit_addr_info.confirmation_status);
        let public_key = musig_id.get_public_key();
        let rune_id = musig_id.get_rune_id();

        let _ = sqlx::query(
            "INSERT INTO gateway.deposit_address (public_key, rune_id, nonce_tweak, address, is_btc, amount, confirmation_status)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (public_key, rune_id, nonce_tweak) DO UPDATE SET confirmation_status = $7",
        )
            .bind(public_key.to_string())
            .bind(rune_id)
            .bind(tweak)
            .bind(deposit_addr_info.address)
            .bind(deposit_addr_info.is_btc)
            .bind(deposit_addr_info.amount as i64)
            .bind(confirmation_status)
            .execute(&self.get_conn().await?)
            .await
            .map_err(|e| DbError::BadRequest(e.to_string()))?;

        Ok(())
    }

    async fn get_confirmation_status(&self, musig_id: &MusigId, tweak: Nonce) -> Result<Option<DepositStatusInfo>, DbError> {
        let public_key = musig_id.get_public_key();
        let rune_id = musig_id.get_rune_id();

        let result: Option<(Json<DepositStatusInfo>, )> = sqlx::query_as(
            "SELECT confirmation_status
            FROM verifier.deposit_address
            WHERE public_key = $1 AND rune_id = $2",
        )
        .bind(public_key.to_string())
        .bind(rune_id)
        .fetch_optional(&self.get_conn().await?)
        .await
        .map_err(|e| DbError::BadRequest(e.to_string()))?;

        Ok(result.map(|deposit_status_info| deposit_status_info.0.0))
    }

    async fn update_confirmation_status(&self, musig_id: &MusigId, tweak: Nonce, confirmation_status: DepositStatusInfo) -> Result<(), DbError> {
        let public_key = musig_id.get_public_key();
        let rune_id = musig_id.get_rune_id();

        let _ = sqlx::query(
            "UPDATE verifier.deposit_address SET confirmation_status = $1 WHERE public_key = $2 AND rune_id = $3",
        )
            .bind(Json(confirmation_status))
            .bind(public_key.to_string())
            .bind(rune_id)
            .execute(&self.get_conn().await?)
            .await
            .map_err(|e| DbError::BadRequest(e.to_string()))?;

        Ok(())
    }
}

