use crate::schemas::user_identifier::{UserUniqueId, UserUuid};
use crate::storage::LocalDbStorage;
use async_trait::async_trait;
use bitcoin::{Address, OutPoint};
use frost::types::TweakBytes;
use persistent_storage::error::DbError;
use persistent_storage::init::{PersistentRepoTrait, StorageHealthcheck};
use serde::{Deserialize, Serialize};
use sqlx::Acquire;
use sqlx::types::Json;
use std::fmt::{Debug, Display, Formatter};
use std::str::FromStr;
use tracing::instrument;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum InnerAddress {
    SparkAddress(String),
    BitcoinAddress(Address),
}

impl Display for InnerAddress {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(
            f,
            "{}",
            match self {
                InnerAddress::SparkAddress(addr) => addr.clone(),
                InnerAddress::BitcoinAddress(addr) => addr.to_string(),
            }
        )
    }
}

impl InnerAddress {
    pub fn from_string_and_type(addr_str: String, is_btc: bool) -> Result<Self, String> {
        if is_btc {
            Address::from_str(&addr_str)
                .map(|addr| InnerAddress::BitcoinAddress(addr.assume_checked()))
                .map_err(|e| format!("Invalid Bitcoin address: {}", e))
        } else {
            Ok(InnerAddress::SparkAddress(addr_str))
        }
    }

    pub fn is_bitcoin(&self) -> bool {
        matches!(self, InnerAddress::BitcoinAddress(_))
    }
}

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

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct DepositAddrInfo {
    pub user_uuid: UserUuid,
    pub rune_id: String,
    pub nonce: TweakBytes,
    pub deposit_address: InnerAddress,
    pub bridge_address: InnerAddress,
    pub is_btc: bool,
    pub deposit_amount: u64,
    pub sats_fee_amount: Option<u64>,
    pub out_point: Option<OutPoint>,
    pub confirmation_status: DepositStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DbDepositAddrInfo {
    pub deposit_address: String,
    pub bridge_address: String,
    pub is_btc: bool,
    pub deposit_amount: u64,
    pub sats_fee_amount: Option<u64>,
    pub out_point: Option<OutPoint>,
    pub confirmation_status: DepositStatus,
}

impl DepositAddrInfo {
    fn to_db_format(&self) -> DbDepositAddrInfo {
        DbDepositAddrInfo {
            deposit_address: self.deposit_address.to_string(),
            bridge_address: self.bridge_address.to_string(),
            is_btc: self.is_btc,
            deposit_amount: self.deposit_amount,
            sats_fee_amount: self.sats_fee_amount,
            out_point: self.out_point,
            confirmation_status: self.confirmation_status.clone(),
        }
    }

    fn from_db_format(
        user_uuid: UserUuid,
        rune_id: String,
        nonce: TweakBytes,
        db_info: DbDepositAddrInfo,
    ) -> Result<Self, String> {
        let deposit_address = InnerAddress::from_string_and_type(db_info.deposit_address, db_info.is_btc)?;

        let bridge_address = InnerAddress::from_string_and_type(db_info.bridge_address, !db_info.is_btc)?;

        Ok(DepositAddrInfo {
            user_uuid,
            rune_id,
            nonce,
            deposit_address,
            bridge_address,
            is_btc: db_info.is_btc,
            deposit_amount: db_info.deposit_amount,
            sats_fee_amount: db_info.sats_fee_amount,
            out_point: db_info.out_point,
            confirmation_status: db_info.confirmation_status,
        })
    }
}

#[async_trait]
pub trait DepositAddressStorage: Send + Sync + StorageHealthcheck {
    async fn get_deposit_addr_info(
        &self,
        user_unique_id: &UserUniqueId,
        tweak: TweakBytes,
    ) -> Result<Option<DepositAddrInfo>, DbError>;
    async fn set_deposit_addr_info(&self, deposit_addr_info: DepositAddrInfo) -> Result<(), DbError>;
    async fn set_confirmation_status_by_out_point(
        &self,
        out_point: OutPoint,
        confirmation_status: DepositStatus,
    ) -> Result<(), DbError>;
    async fn set_sats_fee_amount_by_out_point(&self, out_point: OutPoint, sats_fee_amount: u64) -> Result<(), DbError>;
    async fn set_status_and_fee_amount_by_out_point(
        &self,
        out_point: OutPoint,
        confirmation_status: DepositStatus,
        sats_fee_amount: u64,
    ) -> Result<(), DbError>;
    async fn set_confirmation_status_by_deposit_address(
        &self,
        deposit_address: InnerAddress,
        confirmation_status: DepositStatus,
    ) -> Result<(), DbError>;
}

#[async_trait]
impl DepositAddressStorage for LocalDbStorage {
    #[instrument(level = "trace", skip(self), ret)]
    async fn get_deposit_addr_info(
        &self,
        user_unique_id: &UserUniqueId,
        tweak: TweakBytes,
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
                deposit_address_str,
                bridge_address_str,
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

                let db_info = DbDepositAddrInfo {
                    deposit_address: deposit_address_str,
                    bridge_address: bridge_address_str,
                    is_btc,
                    deposit_amount: deposit_amount as u64,
                    sats_fee_amount: sats_fee_amount.map(|amount| amount as u64),
                    out_point,
                    confirmation_status: confirmation_status.0,
                };

                match DepositAddrInfo::from_db_format(user_uuid, rune_id, tweak, db_info) {
                    Ok(info) => Ok(Some(info)),
                    Err(e) => Err(DbError::BadRequest(format!("Failed to parse address: {}", e))),
                }
            }
            None => Ok(None),
        }
    }

    #[instrument(level = "trace", skip(self), ret)]
    async fn set_deposit_addr_info(&self, deposit_addr_info: DepositAddrInfo) -> Result<(), DbError> {
        let db_info = deposit_addr_info.to_db_format();

        let _ = sqlx::query(
            "INSERT INTO verifier.deposit_address (user_uuid, rune_id, nonce_tweak, deposit_address, bridge_address, is_btc, deposit_amount, sats_fee_amount, confirmation_status, out_point)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            ON CONFLICT (public_key, rune_id, nonce_tweak)
            DO UPDATE SET deposit_address = $3, bridge_address = $4, is_btc = $5, deposit_amount = $6, sats_fee_amount = $7, confirmation_status = $8, out_point = $9",
        )
            .bind(deposit_addr_info.user_uuid)
            .bind(deposit_addr_info.rune_id)
            .bind(deposit_addr_info.nonce)
            .bind(db_info.deposit_address)
            .bind(db_info.bridge_address)
            .bind(db_info.is_btc)
            .bind(db_info.deposit_amount as i64)
            .bind(db_info.sats_fee_amount.map(|amount| amount as i64))
            .bind(Json(db_info.confirmation_status))
            .bind(db_info.out_point.map(|out_point| out_point.to_string()))
            .execute(&self.get_conn().await?)
            .await
            .map_err(|e| DbError::BadRequest(e.to_string()))?;

        Ok(())
    }

    #[instrument(level = "trace", skip(self), ret)]
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

    #[instrument(level = "trace", skip(self), ret)]
    async fn set_sats_fee_amount_by_out_point(&self, out_point: OutPoint, sats_fee_amount: u64) -> Result<(), DbError> {
        let _ = sqlx::query("UPDATE verifier.deposit_address SET sats_fee_amount = $1 WHERE out_point = $2")
            .bind(sats_fee_amount as i64)
            .bind(out_point.to_string())
            .execute(&self.get_conn().await?)
            .await
            .map_err(|e| DbError::BadRequest(e.to_string()))?;

        Ok(())
    }

    #[instrument(level = "trace", skip(self), ret)]
    async fn set_status_and_fee_amount_by_out_point(
        &self,
        out_point: OutPoint,
        confirmation_status: DepositStatus,
        sats_fee_amount: u64,
    ) -> Result<(), DbError> {
        let mut conn = self.postgres_repo.get_conn().await?;
        let mut transaction = conn.begin().await?;

        let _ = sqlx::query("UPDATE verifier.deposit_address SET confirmation_status = $1 WHERE out_point = $2")
            .bind(Json(confirmation_status))
            .bind(out_point.to_string())
            .execute(&mut *transaction)
            .await
            .map_err(|e| DbError::BadRequest(e.to_string()))?;

        let _ = sqlx::query("UPDATE verifier.deposit_address SET sats_fee_amount = $1 WHERE out_point = $2")
            .bind(sats_fee_amount as i64)
            .bind(out_point.to_string())
            .execute(&mut *transaction)
            .await
            .map_err(|e| DbError::BadRequest(e.to_string()))?;

        transaction.commit().await?;

        Ok(())
    }

    #[instrument(level = "trace", skip(self), ret)]
    async fn set_confirmation_status_by_deposit_address(
        &self,
        deposit_address: InnerAddress,
        confirmation_status: DepositStatus,
    ) -> Result<(), DbError> {
        let address_str = deposit_address.to_string();
        let _ = sqlx::query("UPDATE verifier.deposit_address SET confirmation_status = $1 WHERE deposit_address = $2")
            .bind(Json(confirmation_status))
            .bind(address_str)
            .execute(&self.get_conn().await?)
            .await
            .map_err(|e| DbError::BadRequest(e.to_string()))?;

        Ok(())
    }
}
