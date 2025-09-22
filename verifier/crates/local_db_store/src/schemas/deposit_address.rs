use crate::storage::LocalDbStorage;
use async_trait::async_trait;
use bitcoin::OutPoint;
use frost::types::MusigId;
use frost::types::Nonce;
use persistent_storage::error::DbError;
use serde::{Deserialize, Serialize};
use sqlx::types::Json;
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

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct DepositAddrInfo {
    pub musig_id: MusigId,
    pub nonce: Nonce,
    pub deposit_address: String,
    pub bridge_address: String,
    pub is_btc: bool,
    pub deposit_amount: u64,
    pub sats_fee_amount: Option<u64>,
    pub out_point: Option<OutPoint>,
    pub confirmation_status: DepositStatus,
}

#[async_trait]
pub trait DepositAddressStorage {
    async fn get_deposit_addr_info(&self, musig_id: &MusigId, tweak: Nonce) -> Result<Option<DepositAddrInfo>, DbError>;
    async fn set_deposit_addr_info(&self, deposit_addr_info: DepositAddrInfo) -> Result<(), DbError>;
    async fn set_confirmation_status_by_out_point(&self, out_point: OutPoint, confirmation_status: DepositStatus) -> Result<(), DbError>;
    async fn set_sats_fee_amount_by_out_point(&self, out_point: OutPoint, sats_fee_amount: u64) -> Result<(), DbError>;
    async fn set_confirmation_status_by_deposit_address(&self, deposit_address: String, confirmation_status: DepositStatus) -> Result<(), DbError>;
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

        let result: Option<(String, String, bool, i64, Option<i64>, Option<String>, Json<DepositStatus>)> = sqlx::query_as(
            "SELECT deposit_address, bridge_address, is_btc, deposit_amount, sats_fee_amount, out_point, confirmation_status
            FROM verifier.deposit_address
            WHERE public_key = $1 AND rune_id = $2 AND nonce_tweak = $3",
        )
        .bind(public_key.to_string())
        .bind(rune_id)
        .bind(tweak)
        .fetch_optional(&self.get_conn().await?)
        .await
        .map_err(|e| DbError::BadRequest(e.to_string()))?;

        match result {
            Some((deposit_address, bridge_address, is_btc, deposit_amount, sats_fee_amount, out_point_str, confirmation_status)) => {
                let out_point = match out_point_str {
                    Some(out_point_str) => Some(OutPoint::from_str(&out_point_str)
                        .map_err(|e| DbError::DecodeError(format!("Failed to decode out point: {}", e)))?),
                    None => None,
                };

                Ok(Some(DepositAddrInfo {
                    musig_id: musig_id.clone(),
                    nonce: tweak,
                    deposit_address,
                    bridge_address,
                    is_btc,
                    deposit_amount: deposit_amount as u64,
                    sats_fee_amount: sats_fee_amount.map(|sats_fee_amount| sats_fee_amount as u64),
                    out_point,
                    confirmation_status: confirmation_status.0,
                }))
            }
            None => Ok(None),
        }
    }

    async fn set_deposit_addr_info(
        &self,
        deposit_addr_info: DepositAddrInfo,
    ) -> Result<(), DbError> {
        let _ = sqlx::query(
            "INSERT INTO verifier.deposit_address (public_key, rune_id, nonce_tweak, deposit_address, bridge_address, is_btc, deposit_amount, sats_fee_amount, confirmation_status, out_point)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) 
            ON CONFLICT (public_key, rune_id, nonce_tweak) 
            DO UPDATE SET deposit_address = $4, bridge_address = $5, is_btc = $6, deposit_amount = $7, sats_fee_amount = $8, confirmation_status = $9, out_point = $10",
        )
            .bind(deposit_addr_info.musig_id.get_public_key().to_string())
            .bind(deposit_addr_info.musig_id.get_rune_id())
            .bind(deposit_addr_info.nonce)
            .bind(deposit_addr_info.deposit_address)
            .bind(deposit_addr_info.bridge_address)
            .bind(deposit_addr_info.is_btc)
            .bind(deposit_addr_info.deposit_amount as i64)
            .bind(deposit_addr_info.sats_fee_amount.map(|sats_fee_amount| sats_fee_amount as i64))
            .bind(Json(deposit_addr_info.confirmation_status))
            .bind(deposit_addr_info.out_point.map(|out_point| out_point.to_string()))
            .execute(&self.get_conn().await?)
            .await
            .map_err(|e| DbError::BadRequest(e.to_string()))?;

        Ok(())
    }

    async fn set_confirmation_status_by_out_point(&self, out_point: OutPoint, confirmation_status: DepositStatus) -> Result<(), DbError> {
        let _ = sqlx::query(
            "UPDATE verifier.deposit_address SET confirmation_status = $1 WHERE out_point = $2",
        )
            .bind(Json(confirmation_status))
            .bind(out_point.to_string())
            .execute(&self.get_conn().await?)
            .await
            .map_err(|e| DbError::BadRequest(e.to_string()))?;

        Ok(())
    }

    async fn set_sats_fee_amount_by_out_point(&self, out_point: OutPoint, sats_fee_amount: u64) -> Result<(), DbError> {
        let _ = sqlx::query(
            "UPDATE verifier.deposit_address SET sats_fee_amount = $1 WHERE out_point = $2",
        )
            .bind(sats_fee_amount as i64)
            .bind(out_point.to_string())
            .execute(&self.get_conn().await?)
            .await
            .map_err(|e| DbError::BadRequest(e.to_string()))?;

        Ok(())
    }

    async fn set_confirmation_status_by_deposit_address(&self, deposit_address: String, confirmation_status: DepositStatus) -> Result<(), DbError> {
        let _ = sqlx::query(
            "UPDATE verifier.deposit_address SET confirmation_status = $1 WHERE deposit_address = $2",
        )
            .bind(Json(confirmation_status))
            .bind(deposit_address)
            .execute(&self.get_conn().await?)
            .await
            .map_err(|e| DbError::BadRequest(e.to_string()))?;

        Ok(())
    }
}
