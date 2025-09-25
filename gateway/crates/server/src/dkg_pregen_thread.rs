use gateway_config_parser::config::DkgPregenConfig;
use gateway_deposit_verification::aggregator::DepositVerificationAggregator;
use gateway_local_db_store::storage::LocalDbStorage;
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tracing::{debug, error, instrument, trace};

const LOG_PATH: &str = "dkg_pregen_thread";

pub struct DkgPregenThread {}

impl DkgPregenThread {
    #[instrument(skip(local_db))]
    pub async fn spawn_thread(
        task_tracker: &mut TaskTracker,
        local_db: Arc<LocalDbStorage>,
        dkg_pregen_config: DkgPregenConfig,
        cancellation_token: CancellationToken,
    ) {
        task_tracker.spawn(async move {
            trace!("[{LOG_PATH}] Loop spawned..");
            let mut interval = tokio::time::interval(Duration::from_millis(dkg_pregen_config.update_interval_millis));
            'checking_loop: loop {
                tokio::select! {
                    _ = cancellation_token.cancelled() => {
                        debug!("[{LOG_PATH}] Closing [Btc indexer] txs update task, because of cancellation token");
                        break 'checking_loop;
                    },
                    _ = interval.tick() => {
                        match Self::get_possibility_of_update(local_db.clone()).await {
                            Ok(res) => {
                                match res{
                                    (x, true) => {
                                        let _ = Self::pregenerate_shares(local_db.clone()).await.inspect_err(|err|
                                            error!("[{LOG_PATH}] Failed to pregenerate_shares DgkShares: {err}")
                                        );
                                    }
                                    (x, false) => {
                                        trace!("Free dkg values are available: {x}, not performing update for pregenerated DgkShares")
                                    }
                                }
                            }
                            Err(err) => {
                                error!("[{LOG_PATH}] Failed to get possibility_of_update for pregenerated DgkShares: {err}");
                            }
                        }
                    }
                }
            }
        });
    }

    /// Checks available database for availability to generate more dkg pregen values
    pub async fn get_possibility_of_update(local_db: Arc<LocalDbStorage>) -> anyhow::Result<(u64, bool)> {
        Ok((0, false))
    }

    /// Pregenerates shares for dkg state
    pub async fn pregenerate_shares(local_db: Arc<LocalDbStorage>) -> anyhow::Result<()> {
        Ok(())
    }
}
