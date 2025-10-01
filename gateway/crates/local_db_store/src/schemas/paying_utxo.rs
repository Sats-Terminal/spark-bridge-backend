use crate::storage::LocalDbStorage;
use async_trait::async_trait;
use bitcoin::{Txid, Address};
use gateway_rune_transfer::transfer::PayingTransferInput;
use persistent_storage::error::DbError;
use std::str::FromStr;

#[async_trait]
pub trait PayingUtxoStorage: Send + Sync {
    async fn insert_paying_utxo(&self, paying_utxo: PayingTransferInput) -> Result<(), DbError>;
    async fn get_paying_utxo_by_spark_deposit_address(
        &self,
        spark_deposit_address: String,
    ) -> Result<Option<PayingTransferInput>, DbError>;
}

#[async_trait]
impl PayingUtxoStorage for LocalDbStorage {
    async fn insert_paying_utxo(&self, paying_utxo: PayingTransferInput) -> Result<(), DbError> {
        let _ = sqlx::query(
            "INSERT INTO gateway.paying_utxo (txid, vout, spark_deposit_address, sats_amount, none_anyone_can_pay_signature)
            VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(paying_utxo.txid.to_string())
        .bind(paying_utxo.vout as i32)
        .bind(paying_utxo.address.to_string())
        .bind(paying_utxo.sats_amount as i64)
        .bind(paying_utxo.none_anyone_can_pay_signature.to_string())
        .execute(&self.get_conn().await?)
        .await
        .map_err(|e| DbError::BadRequest(format!("Failed to insert paying utxo: {}", e)))?;

        Ok(())
    }

    async fn get_paying_utxo_by_spark_deposit_address(
        &self,
        spark_deposit_address: String,
    ) -> Result<Option<PayingTransferInput>, DbError> {
        let paying_utxo: Option<(String, i32, String, i64, String)> = sqlx::query_as(
            "SELECT txid, vout, spark_deposit_address, sats_amount, none_anyone_can_pay_signature
            FROM gateway.paying_utxo WHERE spark_deposit_address = $1",
        )
        .bind(spark_deposit_address)
        .fetch_optional(&self.get_conn().await?)
        .await
        .map_err(|e| DbError::BadRequest(format!("Failed to get paying utxo by spark deposit address: {}", e)))?;

        match paying_utxo {
            Some((txid, vout, address, sats_amount, none_anyone_can_pay_signature)) => Ok(Some(PayingTransferInput {
                txid: Txid::from_str(&txid).map_err(|e| DbError::BadRequest(format!("Failed to parse txid: {}", e)))?,
                vout: vout as u32,
                address: Address::from_str(&address).unwrap().assume_checked(),
                sats_amount: sats_amount as u64,
                none_anyone_can_pay_signature: none_anyone_can_pay_signature.parse().map_err(|e| {
                    DbError::BadRequest(format!("Failed to parse none anyone can pay signature: {}", e))
                })?,
            })),
            None => Ok(None),
        }
    }
}
