use crate::storage::LocalDbStorage;
use async_trait::async_trait;
use bitcoin::Address;
use frost::types::TweakBytes;
use persistent_storage::error::DbError;
use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use std::collections::HashMap;
use std::fmt::{Debug, Display, Formatter};
use std::str::FromStr;
use std::string::ToString;
use uuid::Uuid;

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

    pub fn to_spark_address(&self) -> Result<String, DbError> {
        match self {
            InnerAddress::SparkAddress(addr) => Ok(addr.clone()),
            InnerAddress::BitcoinAddress(_) => Err(DbError::InvalidData(
                "Cannot convert Bitcoin address to Spark address".to_string(),
            )),
        }
    }

    pub fn to_bitcoin_address(&self) -> Result<Address, DbError> {
        match self {
            InnerAddress::SparkAddress(_) => Err(DbError::InvalidData(
                "Cannot convert Spark address to Bitcoin address".to_string(),
            )),
            InnerAddress::BitcoinAddress(addr) => Ok(addr.clone()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DbDepositAddrInfo {
    pub dkg_share_id: Uuid,
    pub nonce: TweakBytes,
    pub deposit_address: String,
    pub bridge_address: Option<String>,
    pub is_btc: bool,
    pub amount: u64,
    pub confirmation_status: VerifiersResponses,
}

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

    pub fn empty() -> Self {
        VerifiersResponses {
            responses: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct DepositAddrInfo {
    pub dkg_share_id: Uuid,
    pub nonce: TweakBytes,
    pub deposit_address: InnerAddress,
    pub bridge_address: Option<InnerAddress>,
    pub is_btc: bool,
    pub amount: u64,
    pub confirmation_status: VerifiersResponses,
}

impl DepositAddrInfo {
    fn to_db_format(&self) -> DbDepositAddrInfo {
        DbDepositAddrInfo {
            dkg_share_id: self.dkg_share_id,
            nonce: self.nonce,
            deposit_address: self.deposit_address.to_string(),
            bridge_address: self.bridge_address.as_ref().map(|addr| addr.to_string()),
            is_btc: self.is_btc,
            amount: self.amount,
            confirmation_status: self.confirmation_status.clone(),
        }
    }

    fn from_db_format(db_info: DbDepositAddrInfo) -> Result<Self, String> {
        let deposit_address = InnerAddress::from_string_and_type(db_info.deposit_address, db_info.is_btc)?;

        let bridge_address = match db_info.bridge_address {
            Some(addr_str) => Some(InnerAddress::from_string_and_type(addr_str, !db_info.is_btc)?),
            None => None,
        };

        Ok(DepositAddrInfo {
            dkg_share_id: db_info.dkg_share_id,
            nonce: db_info.nonce,
            deposit_address,
            bridge_address,
            is_btc: db_info.is_btc,
            amount: db_info.amount,
            confirmation_status: db_info.confirmation_status,
        })
    }
}

#[async_trait]
pub trait DepositAddressStorage: Send + Sync {
    async fn insert_deposit_addr_info(&self, deposit_addr_info: DepositAddrInfo) -> Result<(), DbError>;
    async fn set_confirmation_status_by_deposit_address(
        &self,
        address: &InnerAddress,
        confirmation_status: VerifiersResponses,
    ) -> Result<(), DbError>;
    async fn get_row_by_deposit_address(&self, address: &InnerAddress) -> Result<Option<DepositAddrInfo>, DbError>;
    async fn update_confirmation_status_by_deposit_address(
        &self,
        deposit_address: &InnerAddress,
        verifier_id: u16,
        verifier_response: DepositStatus,
    ) -> Result<(), DbError>;
    async fn update_bridge_address_by_deposit_address(
        &self,
        deposit_address: &InnerAddress,
        bridge_address: &InnerAddress,
    ) -> Result<(), DbError>;
}

#[async_trait]
impl DepositAddressStorage for LocalDbStorage {
    #[tracing::instrument(level = "trace", skip_all)]
    async fn insert_deposit_addr_info(&self, deposit_addr_info: DepositAddrInfo) -> Result<(), DbError> {
        let db_info = deposit_addr_info.to_db_format();

        let _ = sqlx::query(
            "INSERT INTO gateway.deposit_address (nonce_tweak, dkg_share_id, deposit_address, bridge_address, is_btc, amount, confirmation_status)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (nonce_tweak) DO UPDATE SET confirmation_status = $7",
        )
            .bind(deposit_addr_info.nonce)
            .bind(deposit_addr_info.dkg_share_id)
            .bind(db_info.deposit_address)
            .bind(db_info.bridge_address)
            .bind(db_info.is_btc)
            .bind(db_info.amount as i64)
            .bind(Json(db_info.confirmation_status))
            .execute(&self.get_conn().await?)
            .await
            .map_err(|e| DbError::BadRequest(e.to_string()))?;

        Ok(())
    }

    #[tracing::instrument(level = "trace", skip_all)]
    async fn set_confirmation_status_by_deposit_address(
        &self,
        address: &InnerAddress,
        confirmation_status: VerifiersResponses,
    ) -> Result<(), DbError> {
        let address_str = address.to_string();
        let _ = sqlx::query("UPDATE gateway.deposit_address SET confirmation_status = $1 WHERE deposit_address = $2")
            .bind(Json(confirmation_status))
            .bind(address_str)
            .execute(&self.get_conn().await?)
            .await
            .map_err(|e| DbError::BadRequest(e.to_string()))?;

        Ok(())
    }

    #[tracing::instrument(level = "trace", skip_all)]
    async fn get_row_by_deposit_address(
        &self,
        deposit_address: &InnerAddress,
    ) -> Result<Option<DepositAddrInfo>, DbError> {
        let address_str = deposit_address.to_string();

        let result: Option<(
            Uuid,
            TweakBytes,
            String,
            Option<String>,
            bool,
            i64,
            Json<VerifiersResponses>,
        )> = sqlx::query_as(
            "SELECT dkg_share_id, nonce_tweak, deposit_address, bridge_address, is_btc, amount, confirmation_status
            FROM gateway.deposit_address WHERE deposit_address = $1",
        )
        .bind(address_str)
        .fetch_optional(&self.get_conn().await?)
        .await
        .map_err(|e| DbError::BadRequest(e.to_string()))?;

        match result {
            Some((
                dkg_share_id,
                nonce_tweak,
                deposit_address_str,
                bridge_address_str,
                is_btc,
                amount,
                confirmation_status,
            )) => {
                let db_info = DbDepositAddrInfo {
                    dkg_share_id,
                    nonce: nonce_tweak,
                    deposit_address: deposit_address_str,
                    bridge_address: bridge_address_str,
                    is_btc,
                    amount: amount as u64,
                    confirmation_status: confirmation_status.0,
                };

                match DepositAddrInfo::from_db_format(db_info) {
                    Ok(info) => Ok(Some(info)),
                    Err(e) => Err(DbError::BadRequest(format!("Failed to parse address: {}", e))),
                }
            }
            None => Ok(None),
        }
    }

    #[tracing::instrument(level = "trace", skip_all)]
    async fn update_confirmation_status_by_deposit_address(
        &self,
        deposit_address: &InnerAddress,
        verifier_id: u16,
        verifier_response: DepositStatus,
    ) -> Result<(), DbError> {
        let address_str = deposit_address.to_string();
        let mut tx = self.get_conn().await?.begin().await?;

        let response: Option<(Json<VerifiersResponses>,)> = sqlx::query_as(
            "SELECT confirmation_status
            FROM gateway.deposit_address
            WHERE deposit_address = $1
            FOR UPDATE",
        )
        .bind(address_str.clone())
        .fetch_optional(&mut *tx)
        .await?;

        let mut confirmation_status = match response {
            Some((confirmation_status,)) => confirmation_status.0,
            None => {
                return Err(DbError::NotFound(format!(
                    "Confirmation status not found for address: {}",
                    address_str
                )));
            }
        };

        confirmation_status.responses.insert(verifier_id, verifier_response);

        let _ = sqlx::query(
            "UPDATE gateway.deposit_address
            SET confirmation_status = $1
            WHERE deposit_address = $2",
        )
        .bind(Json(confirmation_status))
        .bind(address_str)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(())
    }

    #[tracing::instrument(level = "trace", skip_all)]
    async fn update_bridge_address_by_deposit_address(
        &self,
        deposit_address: &InnerAddress,
        bridge_address: &InnerAddress,
    ) -> Result<(), DbError> {
        let deposit_addr_str = deposit_address.to_string();
        let bridge_addr_str = bridge_address.to_string();

        let _ = sqlx::query(
            "UPDATE gateway.deposit_address
            SET bridge_address = $1
            WHERE deposit_address = $2",
        )
        .bind(bridge_addr_str)
        .bind(deposit_addr_str)
        .execute(&self.get_conn().await?)
        .await?;

        Ok(())
    }
}
