use std::{
    fmt::{Debug, Display, Formatter},
    str::FromStr,
};

use async_trait::async_trait;
use bitcoin::{Address, OutPoint};
use frost::types::TweakBytes;
use persistent_storage::error::DbError;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use tracing::instrument;
use uuid::Uuid;

use crate::storage::LocalDbStorage;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum InnerAddress {
    SparkAddress(String),
    BitcoinAddress(Address),
}

impl Display for InnerAddress {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
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

#[derive(Debug, Clone, Deserialize, Serialize, Eq, PartialEq, sqlx::Type, Hash)]
#[sqlx(rename_all = "snake_case", type_name = "DEPOSIT_STATUS")]
pub enum DepositStatus {
    Pending,
    Confirmed,
    Failed,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
#[serde(rename_all = "lowercase")]
pub enum FeePayment {
    Btc(OutPoint),
    Spark(String),
}

impl std::fmt::Display for FeePayment {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            FeePayment::Btc(outpoint) => write!(f, "{}", outpoint),
            FeePayment::Spark(spark_out) => write!(f, "{}", spark_out),
        }
    }
}

impl std::str::FromStr for FeePayment {
    type Err = DbError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some((txid_str, vout_str)) = s.split_once(':')
            && let (Ok(txid), Ok(vout)) = (bitcoin::Txid::from_str(txid_str), vout_str.parse::<u32>())
        {
            return Ok(FeePayment::Btc(bitcoin::OutPoint { txid, vout }));
        }
        Ok(FeePayment::Spark(s.to_owned()))
    }
}

#[derive(Debug, Clone)]
pub struct DepositAddrInfo {
    pub dkg_share_id: Uuid,
    pub nonce: TweakBytes,
    pub deposit_address: InnerAddress,
    pub bridge_address: InnerAddress,
    pub is_btc: bool,
    pub deposit_amount: u64,
    pub token: String,
    pub sats_amount: Option<u64>,
    pub outpoint: Option<OutPoint>,
    pub fee_payment: Option<FeePayment>,
    pub fee_payment_status: DepositStatus,
    pub confirmation_status: DepositStatus,
    pub error_details: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
struct DbDepositAddrInfo {
    pub dkg_share_id: Uuid,
    #[sqlx(rename = "nonce_tweak")]
    pub nonce: TweakBytes,
    pub deposit_address: String,
    pub bridge_address: String,
    pub is_btc: bool,
    pub deposit_amount: i64,
    pub token: String,
    pub sats_amount: Option<i64>,
    pub outpoint: Option<String>,
    pub fee_payment: Option<String>,
    pub fee_payment_status: DepositStatus,
    pub confirmation_status: DepositStatus,
    pub error_details: Option<String>,
}

impl DepositAddrInfo {
    fn to_db_format(&self) -> DbDepositAddrInfo {
        DbDepositAddrInfo {
            dkg_share_id: self.dkg_share_id,
            nonce: self.nonce,
            deposit_address: self.deposit_address.to_string(),
            bridge_address: self.bridge_address.to_string(),
            is_btc: self.is_btc,
            token: self.token.clone(),
            deposit_amount: self.deposit_amount as i64,
            sats_amount: self.sats_amount.map(|amount| amount as i64),
            outpoint: self.outpoint.map(|outpoint| outpoint.to_string()),
            fee_payment: self.fee_payment.clone().map(|outpoint| outpoint.to_string()),
            fee_payment_status: self.fee_payment_status.clone(),
            confirmation_status: self.confirmation_status.clone(),
            error_details: self.error_details.clone(),
        }
    }

    fn from_db_format(db_info: DbDepositAddrInfo) -> Result<Self, DbError> {
        let deposit_address = InnerAddress::from_string_and_type(db_info.deposit_address, db_info.is_btc)
            .map_err(|e| DbError::DecodeError(format!("Failed to parse deposit address: {}", e)))?;

        let bridge_address = InnerAddress::from_string_and_type(db_info.bridge_address, !db_info.is_btc)
            .map_err(|e| DbError::DecodeError(format!("Failed to parse bridge address: {}", e)))?;

        let outpoint = match db_info.outpoint {
            Some(outpoint) => Some(
                OutPoint::from_str(&outpoint)
                    .map_err(|e| DbError::DecodeError(format!("Failed to parse outpoint: {}", e)))?,
            ),
            None => None,
        };

        let fee_payment: Option<FeePayment> = match db_info.fee_payment {
            Some(payment) => Some(
                FeePayment::from_str(&payment)
                    .map_err(|e| DbError::DecodeError(format!("Failed to parse fee payment: {}", e)))?,
            ),
            None => None,
        };

        Ok(DepositAddrInfo {
            dkg_share_id: db_info.dkg_share_id,
            nonce: db_info.nonce,
            deposit_address,
            bridge_address,
            is_btc: db_info.is_btc,
            token: db_info.token,
            deposit_amount: db_info.deposit_amount as u64,
            sats_amount: db_info.sats_amount.map(|amount| amount as u64),
            outpoint,
            fee_payment: fee_payment.clone(),
            fee_payment_status: db_info.fee_payment_status,
            confirmation_status: db_info.confirmation_status,
            error_details: db_info.error_details,
        })
    }
}

#[async_trait]
pub trait DepositAddressStorage: Send + Sync {
    async fn get_deposit_addr_info_by_confirmed_fee_payment(
        &self,
        payment: FeePayment,
    ) -> Result<DepositAddrInfo, DbError>;
    async fn insert_deposit_addr_info(&self, deposit_addr_info: DepositAddrInfo) -> Result<(), DbError>;
    async fn set_confirmation_status_by_out_point(
        &self,
        outpoint: OutPoint,
        confirmation_status: DepositStatus,
        error_details: Option<String>,
    ) -> Result<(), DbError>;
    async fn set_fee_status_by_payment(
        &self,
        payment: FeePayment,
        status: DepositStatus,
        err: Option<String>,
    ) -> Result<(), DbError>;
    async fn set_sats_amount_by_out_point(&self, outpoint: OutPoint, sats_amount: u64) -> Result<(), DbError>;
    async fn set_confirmation_status_by_deposit_address(
        &self,
        deposit_address: InnerAddress,
        confirmation_status: DepositStatus,
        error_details: Option<String>,
    ) -> Result<(), DbError>;
}

#[async_trait]
impl DepositAddressStorage for LocalDbStorage {
    #[instrument(level = "trace", skip(self), ret)]
    async fn get_deposit_addr_info_by_confirmed_fee_payment(
        &self,
        payment: FeePayment,
    ) -> Result<DepositAddrInfo, DbError> {
        let result: Option<DbDepositAddrInfo> = sqlx::query_as(
            "SELECT dkg_share_id, nonce_tweak, deposit_address, bridge_address, is_btc, deposit_amount, token, sats_amount, outpoint, fee_payment, fee_payment_status, confirmation_status, error_details
            FROM verifier.deposit_address
            WHERE fee_payment = $1 and fee_payment_status = 'confirmed'",
        )
        .bind(payment.to_string())
        .fetch_optional(&self.get_conn().await?)
        .await
        .map_err(|e| DbError::BadRequest(e.to_string()))?;

        match result {
            Some(db_info) => Ok(DepositAddrInfo::from_db_format(db_info)?),
            None => Err(DbError::NotFound(format!(
                "Deposit address info not found for payment: {}",
                payment
            ))),
        }
    }

    #[instrument(level = "trace", skip(self), ret)]
    async fn insert_deposit_addr_info(&self, deposit_addr_info: DepositAddrInfo) -> Result<(), DbError> {
        let db_info = deposit_addr_info.to_db_format();
        let _ = sqlx::query(
            "INSERT INTO verifier.deposit_address (dkg_share_id, nonce_tweak, deposit_address, bridge_address, is_btc, deposit_amount, token, sats_amount, confirmation_status, outpoint, fee_payment, fee_payment_status, error_details)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)",
        )
            .bind(db_info.dkg_share_id)
            .bind(db_info.nonce)
            .bind(db_info.deposit_address)
            .bind(db_info.bridge_address)
            .bind(db_info.is_btc)
            .bind(db_info.deposit_amount)
            .bind(db_info.token)
            .bind(db_info.sats_amount)
            .bind(db_info.confirmation_status)
            .bind(db_info.outpoint.map(|outpoint| outpoint.to_string()))
            .bind(db_info.fee_payment.map(|fee_payment| fee_payment.to_string()))
            .bind(db_info.fee_payment_status)
            .bind(db_info.error_details)
            .execute(&self.get_conn().await?)
            .await
            .map_err(|e| DbError::BadRequest(e.to_string()))?;

        Ok(())
    }

    #[instrument(level = "trace", skip(self), ret)]
    async fn set_confirmation_status_by_out_point(
        &self,
        outpoint: OutPoint,
        confirmation_status: DepositStatus,
        error_details: Option<String>,
    ) -> Result<(), DbError> {
        let _ = sqlx::query(
            "UPDATE verifier.deposit_address 
            SET confirmation_status = $1, error_details = $2 
            WHERE outpoint = $3 and fee_payment_status = 'confirmed'",
        )
        .bind(confirmation_status)
        .bind(error_details)
        .bind(outpoint.to_string())
        .execute(&self.get_conn().await?)
        .await
        .map_err(|e| DbError::BadRequest(e.to_string()))?;

        Ok(())
    }

    #[instrument(level = "trace", skip(self), ret)]
    async fn set_fee_status_by_payment(
        &self,
        payment: FeePayment,
        status: DepositStatus,
        err: Option<String>,
    ) -> Result<(), DbError> {
        let _ = sqlx::query(
            "UPDATE verifier.deposit_address 
            SET fee_payment_status = $1, error_details = $2 
            WHERE fee_payment = $3",
        )
        .bind(status)
        .bind(err)
        .bind(payment.to_string())
        .execute(&self.get_conn().await?)
        .await
        .map_err(|e| DbError::BadRequest(e.to_string()))?;

        Ok(())
    }

    #[instrument(level = "trace", skip(self), ret)]
    async fn set_sats_amount_by_out_point(&self, outpoint: OutPoint, sats_amount: u64) -> Result<(), DbError> {
        let _ = sqlx::query(
            "UPDATE verifier.deposit_address 
            SET sats_amount = $1 
            WHERE outpoint = $2",
        )
        .bind(sats_amount as i64)
        .bind(outpoint.to_string())
        .execute(&self.get_conn().await?)
        .await
        .map_err(|e| DbError::BadRequest(e.to_string()))?;

        Ok(())
    }

    #[instrument(level = "trace", skip(self), ret)]
    async fn set_confirmation_status_by_deposit_address(
        &self,
        deposit_address: InnerAddress,
        confirmation_status: DepositStatus,
        error_details: Option<String>,
    ) -> Result<(), DbError> {
        let _ = sqlx::query(
            "UPDATE verifier.deposit_address 
            SET confirmation_status = $1, error_details = $2 
            WHERE deposit_address = $3",
        )
        .bind(confirmation_status)
        .bind(error_details)
        .bind(deposit_address.to_string())
        .execute(&self.get_conn().await?)
        .await
        .map_err(|e| DbError::BadRequest(e.to_string()))?;

        Ok(())
    }
}
