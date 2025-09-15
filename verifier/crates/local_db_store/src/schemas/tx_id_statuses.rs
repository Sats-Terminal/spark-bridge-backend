use crate::storage::LocalDbStorage;
use async_trait::async_trait;
use bitcoin::Txid;
use btc_resp_aggregator::traits::{BtcTxIdStatusStorage, TxIdStatusValue, TxidStatus};
use global_utils::common_types::UrlWrapped;
use persistent_storage::error::DbError;

#[async_trait]
impl BtcTxIdStatusStorage for LocalDbStorage {
    async fn get_tx_id_value(&self, tx_id: Txid) -> Result<Option<TxIdStatusValue>, DbError> {
        let result: Option<(TxidStatus, UrlWrapped)> = sqlx::query_as(
            "SELECT tx_response_state, gateway_loopback_addr FROM verifier.tx_ids_statuses WHERE tx_id = $1",
        )
        .bind(tx_id.to_string())
        .fetch_optional(&self.get_conn().await?)
        .await
        .map_err(|e| DbError::BadRequest(e.to_string()))?;
        Ok(result.map(|x| TxIdStatusValue {
            gateway_loopback_addr: x.1,
            status: x.0,
        }))
    }

    async fn set_tx_id_value(&self, tx_id: Txid, update: &TxIdStatusValue) -> Result<(), DbError> {
        sqlx::query(
            "INSERT INTO verifier.tx_ids_statuses (tx_id, gateway_loopback_addr, tx_response_state)
             VALUES ($1, $2, $3)
             ON CONFLICT (tx_id) DO UPDATE SET gateway_loopback_addr = $2, tx_response_state = $3",
        )
        .bind(tx_id.to_string())
        .bind(&update.gateway_loopback_addr)
        .bind(&update.status)
        .execute(&self.get_conn().await?)
        .await
        .map_err(|e| DbError::BadRequest(e.to_string()))?;
        Ok(())
    }

    async fn set_tx_id_status(&self, tx_id: Txid, status: &TxidStatus) -> Result<(), DbError> {
        sqlx::query("UPDATE verifier.tx_ids_statuses SET tx_response_state = $2 WHERE tx_id = $1")
            .bind(tx_id.to_string())
            .bind(status)
            .execute(&self.get_conn().await?)
            .await
            .map_err(|e| DbError::BadRequest(e.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod testing_db_interaction {
    use super::*;
    use std::str::FromStr;

    use global_utils::common_types::Url;
    use global_utils::logger::{LoggerGuard, init_logger};
    use persistent_storage::init::{PostgresPool, PostgresRepo};
    use std::sync::{Arc, LazyLock};

    static TEST_LOGGER: LazyLock<LoggerGuard> = LazyLock::new(|| init_logger());
    pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn test_setting(db: PostgresPool) -> anyhow::Result<()> {
        let _ = *TEST_LOGGER;
        let storage = Arc::new(LocalDbStorage {
            postgres_repo: PostgresRepo { pool: db },
        });

        let (tx_id1, value1) = (
            Txid::from_str("e3fb40df17e1852bb9bb20acfab148772904322d77970d2860937f232c726148")?,
            TxIdStatusValue {
                gateway_loopback_addr: UrlWrapped(Url::from_str("http://example.com")?),
                status: TxidStatus::Created,
            },
        );

        let (tx_id2, value2) = (
            Txid::from_str("dcb3911e481fd00846e2bf9ae7911e2e3be79397b285d59a031f512d30ddb3f0")?,
            TxIdStatusValue {
                gateway_loopback_addr: UrlWrapped(Url::from_str("http://example2.com")?),
                status: TxidStatus::Received,
            },
        );
        storage.set_tx_id_value(tx_id1, &value1).await?;
        storage.set_tx_id_value(tx_id2, &value2).await?;

        assert_eq!(storage.get_tx_id_value(tx_id1).await?, Some(value1.clone()));
        assert_eq!(storage.get_tx_id_value(tx_id2).await?, Some(value2.clone()));

        storage.set_tx_id_value(tx_id1, &value2).await?;
        storage.set_tx_id_value(tx_id2, &value1).await?;

        assert_eq!(storage.get_tx_id_value(tx_id1).await?, Some(value2.clone()));
        assert_eq!(storage.get_tx_id_value(tx_id2).await?, Some(value1.clone()));
        Ok(())
    }

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn test_statuses_change(db: PostgresPool) -> anyhow::Result<()> {
        let _ = *TEST_LOGGER;
        let storage = Arc::new(LocalDbStorage {
            postgres_repo: PostgresRepo { pool: db },
        });

        let (tx_id1, mut value1) = (
            Txid::from_str("e3fb40df17e1852bb9bb20acfab148772904322d77970d2860937f232c726148")?,
            TxIdStatusValue {
                gateway_loopback_addr: UrlWrapped(Url::from_str("http://example.com")?),
                status: TxidStatus::Created,
            },
        );
        storage.set_tx_id_value(tx_id1, &value1).await?;
        assert_eq!(storage.get_tx_id_value(tx_id1).await?, Some(value1.clone()));

        value1.status = TxidStatus::Processing;
        storage.set_tx_id_status(tx_id1, &TxidStatus::Processing).await?;
        assert_eq!(storage.get_tx_id_value(tx_id1).await?, Some(value1.clone()));

        value1.status = TxidStatus::Received;
        storage.set_tx_id_status(tx_id1, &TxidStatus::Received).await?;
        assert_eq!(storage.get_tx_id_value(tx_id1).await?, Some(value1.clone()));
        Ok(())
    }
}
