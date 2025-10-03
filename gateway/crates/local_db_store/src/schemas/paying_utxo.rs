use crate::storage::LocalDbStorage;
use async_trait::async_trait;
use bitcoin::{Address, Txid};
use gateway_rune_transfer::transfer::PayingTransferInput;
use persistent_storage::error::DbError;
use persistent_storage::init::StorageHealthcheck;
use std::str::FromStr;
use global_utils::conversion::decode_address;
use tracing::instrument;

#[async_trait]
pub trait PayingUtxoStorage: Send + Sync + StorageHealthcheck {
    async fn insert_paying_utxo(&self, paying_utxo: PayingTransferInput) -> Result<(), DbError>;
    async fn get_paying_utxo_by_btc_exit_address(
        &self,
        btc_exit_address: Address,
    ) -> Result<Option<PayingTransferInput>, DbError>;
}

#[async_trait]
impl PayingUtxoStorage for LocalDbStorage {
    #[instrument(level = "trace", skip_all)]
    async fn insert_paying_utxo(&self, paying_utxo: PayingTransferInput) -> Result<(), DbError> {
        let _ = sqlx::query(
            "INSERT INTO gateway.paying_utxo (txid, vout, btc_exit_address, sats_amount, none_anyone_can_pay_signature)
            VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(paying_utxo.txid.to_string())
        .bind(paying_utxo.vout as i32)
        .bind(paying_utxo.btc_exit_address.to_string())
        .bind(paying_utxo.sats_amount as i64)
        .bind(paying_utxo.none_anyone_can_pay_signature.to_string())
        .execute(&self.get_conn().await?)
        .await
        .map_err(|e| DbError::BadRequest(format!("Failed to insert paying utxo: {}", e)))?;

        Ok(())
    }

    #[instrument(level = "trace", skip_all)]
    async fn get_paying_utxo_by_btc_exit_address(
        &self,
        btc_exit_address: Address,
    ) -> Result<Option<PayingTransferInput>, DbError> {
        let paying_utxo: Option<(String, i32, String, i64, String)> = sqlx::query_as(
            "SELECT txid, vout, btc_exit_address, sats_amount, none_anyone_can_pay_signature
            FROM gateway.paying_utxo WHERE btc_exit_address = $1",
        )
        .bind(btc_exit_address.to_string())
        .fetch_optional(&self.get_conn().await?)
        .await
        .map_err(|e| DbError::BadRequest(format!("Failed to get paying utxo by spark deposit address: {}", e)))?;

        match paying_utxo {
            Some((txid, vout, address, sats_amount, none_anyone_can_pay_signature)) => Ok(Some(PayingTransferInput {
                txid: Txid::from_str(&txid).map_err(|e| DbError::BadRequest(format!("Failed to parse txid: {}", e)))?,
                vout: vout as u32,
                btc_exit_address: Address::from_str(&address)
                    .map_err(|e| DbError::BadRequest(format!("Failed to parse address: {}", e)))?
                    .assume_checked(),
                sats_amount: sats_amount as u64,
                none_anyone_can_pay_signature: none_anyone_can_pay_signature.parse().map_err(|e| {
                    DbError::DecodeError(format!("Failed to parse none anyone can pay signature: {}", e))
                })?,
            })),
            None => Ok(None),
        }
    }
}
