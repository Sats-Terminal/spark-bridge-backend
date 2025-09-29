use crate::schemas::user_identifier::{UserUniqueId, UserUuid};
use crate::storage::LocalDbStorage;
use async_trait::async_trait;
use bitcoin::OutPoint;
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
    pub user_uuid: UserUuid,
    pub rune_id: String,
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
    async fn get_deposit_addr_info(
        &self,
        user_unique_id: &UserUniqueId,
        tweak: Nonce,
    ) -> Result<Option<DepositAddrInfo>, DbError>;
    async fn set_deposit_addr_info(&self, deposit_addr_info: DepositAddrInfo) -> Result<(), DbError>;
    async fn set_confirmation_status_by_out_point(
        &self,
        out_point: OutPoint,
        confirmation_status: DepositStatus,
    ) -> Result<(), DbError>;
    async fn set_sats_fee_amount_by_out_point(&self, out_point: OutPoint, sats_fee_amount: u64) -> Result<(), DbError>;
    async fn set_confirmation_status_by_deposit_address(
        &self,
        deposit_address: String,
        confirmation_status: DepositStatus,
    ) -> Result<(), DbError>;
}

#[async_trait]
impl DepositAddressStorage for LocalDbStorage {
    async fn get_deposit_addr_info(
        &self,
        user_unique_id: &UserUniqueId,
        tweak: Nonce,
    ) -> Result<Option<DepositAddrInfo>, DbError> {
        let result: Option<(UserUuid, String, String, String, bool, i64, Option<i64>, Option<String>, Json<DepositStatus>)> = sqlx::query_as(
            "SELECT user_uuid, rune_id, deposit_address, bridge_address, is_btc, deposit_amount, sats_fee_amount, out_point, confirmation_status
            FROM verifier.deposit_address
            WHERE user_uuid = $1 AND rune_id = $2 AND nonce_tweak = $3",
        )
        .bind(user_unique_id.uuid)
        .bind(&user_unique_id.rune_id)
        .bind(tweak)
        .fetch_optional(&self.get_conn().await?)
        .await
        .map_err(|e| DbError::BadRequest(e.to_string()))?;

        match result {
            Some((
                user_uuid,
                rune_id,
                deposit_address,
                bridge_address,
                is_btc,
                deposit_amount,
                sats_fee_amount,
                out_point_str,
                confirmation_status,
            )) => {
                let out_point = match out_point_str {
                    Some(out_point_str) => Some(
                        OutPoint::from_str(&out_point_str)
                            .map_err(|e| DbError::DecodeError(format!("Failed to decode out point: {}", e)))?,
                    ),
                    None => None,
                };

                Ok(Some(DepositAddrInfo {
                    user_uuid,
                    rune_id,
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

    async fn set_deposit_addr_info(&self, deposit_addr_info: DepositAddrInfo) -> Result<(), DbError> {
        let _ = sqlx::query(
            "INSERT INTO verifier.deposit_address (user_uuid, rune_id, nonce_tweak, deposit_address, bridge_address, is_btc, deposit_amount, sats_fee_amount, confirmation_status, out_point)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            ON CONFLICT (public_key, rune_id, nonce_tweak) 
            DO UPDATE SET deposit_address = $3, bridge_address = $4, is_btc = $5, deposit_amount = $6, sats_fee_amount = $7, confirmation_status = $8, out_point = $9",
        )
            .bind(deposit_addr_info.user_uuid)
            .bind(deposit_addr_info.rune_id)
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

    async fn set_confirmation_status_by_out_point(
        &self,
        out_point: OutPoint,
        confirmation_status: DepositStatus,
    ) -> Result<(), DbError> {
        let _ = sqlx::query("UPDATE verifier.deposit_address SET confirmation_status = $1 WHERE out_point = $2")
            .bind(Json(confirmation_status))
            .bind(out_point.to_string())
            .execute(&self.get_conn().await?)
            .await
            .map_err(|e| DbError::BadRequest(e.to_string()))?;

        Ok(())
    }

    async fn set_sats_fee_amount_by_out_point(&self, out_point: OutPoint, sats_fee_amount: u64) -> Result<(), DbError> {
        let _ = sqlx::query("UPDATE verifier.deposit_address SET sats_fee_amount = $1 WHERE out_point = $2")
            .bind(sats_fee_amount as i64)
            .bind(out_point.to_string())
            .execute(&self.get_conn().await?)
            .await
            .map_err(|e| DbError::BadRequest(e.to_string()))?;

        Ok(())
    }

    async fn set_confirmation_status_by_deposit_address(
        &self,
        deposit_address: String,
        confirmation_status: DepositStatus,
    ) -> Result<(), DbError> {
        let _ = sqlx::query("UPDATE verifier.deposit_address SET confirmation_status = $1 WHERE deposit_address = $2")
            .bind(Json(confirmation_status))
            .bind(deposit_address)
            .execute(&self.get_conn().await?)
            .await
            .map_err(|e| DbError::BadRequest(e.to_string()))?;

        Ok(())
    }
}
