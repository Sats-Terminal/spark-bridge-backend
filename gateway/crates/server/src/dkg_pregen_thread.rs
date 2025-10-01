use frost::aggregator::FrostAggregator;
use gateway_config_parser::config::DkgPregenConfig;
use gateway_deposit_verification::aggregator::DepositVerificationAggregator;
use gateway_local_db_store::schemas::dkg_share::DkgShareGenerate;
use gateway_local_db_store::storage::LocalDbStorage;
use global_utils::common_types::get_uuid;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tracing::{debug, error, instrument, trace};

const LOG_PATH: &str = "dkg_pregen_thread";

static EPOCH: AtomicU64 = AtomicU64::new(0);

pub struct DkgPregenThread {}

struct UpdatePossibility {
    dkg_available: u64,
    finalized_dkg_available: u64,
}

type Storage = Arc<LocalDbStorage>;
type Aggreagator = Arc<FrostAggregator>;

impl DkgPregenThread {
    #[instrument(skip(local_db))]
    pub async fn spawn_thread(
        task_tracker: &mut TaskTracker,
        local_db: Storage,
        dkg_pregen_config: DkgPregenConfig,
        frost_aggregator: Aggreagator,
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
                        Self::perform_update(local_db.clone(), frost_aggregator.clone(), &dkg_pregen_config).await;
                    }
                }
            }
        });
    }

    pub async fn perform_update(local_db: Storage, dkg_aggregator: Aggreagator, dkg_pregen_config: &DkgPregenConfig) {
        match Self::get_possible_update_info(local_db.clone()).await {
            Ok(UpdatePossibility {
                dkg_available,
                finalized_dkg_available,
            }) => match Self::get_update_decision(dkg_available, finalized_dkg_available, &dkg_pregen_config) {
                0 => {
                    trace!(
                        "Free dkg values are available: {dkg_available}, \
                                        finalized dkgs: {finalized_dkg_available}, \
                                        not performing update for pregenerated DgkShares"
                    )
                }
                amount_to_gen => {
                    let _ = Self::pregenerate_shares(local_db.clone(), dkg_aggregator.clone(), amount_to_gen)
                        .await
                        .inspect_err(|err| error!("[{LOG_PATH}] Failed to pregenerate_shares DgkShares: {err}"));
                }
            },
            Err(err) => {
                error!("[{LOG_PATH}] Failed to get possibility_of_update for pregenerated DgkShares: {err}");
            }
        }
    }

    async fn get_possible_update_info(local_db: Storage) -> anyhow::Result<UpdatePossibility> {
        Ok(UpdatePossibility {
            dkg_available: local_db.count_unused_dkg_shares().await?,
            finalized_dkg_available: local_db.count_unused_finalized_dkg_shares().await?,
        })
    }

    /// Checks availability to generate more dkg pregen values
    fn get_update_decision(_dkg_available: u64, finalized_dkg_available: u64, config: &DkgPregenConfig) -> u64 {
        if finalized_dkg_available < config.min_threshold {
            config.min_threshold - finalized_dkg_available
        } else {
            0
        }
    }

    /// Pregenerates shares for dkg state
    async fn pregenerate_shares(local_db: Storage, dkg_aggregator: Aggreagator, amount: u64) -> anyhow::Result<()> {
        let mut join_set = JoinSet::new();
        trace!("[{LOG_PATH}] Pregenerating epoch {}", EPOCH.load(Ordering::SeqCst));
        for _ in 0..amount {
            join_set.spawn({
                let (local_db, aggregator) = (local_db.clone(), dkg_aggregator.clone());
                async move { Self::pregenerate_share(local_db, aggregator).await }
            });
        }
        let _ = join_set.join_all().await;
        EPOCH.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    #[instrument(level = "trace", skip(local_db, aggregator), fields(epoch=EPOCH.load(Ordering::SeqCst)), err)]
    async fn pregenerate_share(local_db: Storage, aggregator: Aggreagator) -> anyhow::Result<()> {
        let initialized_entity = local_db.generate_dkg_share_entity().await?;
        aggregator.run_dkg_flow(&initialized_entity).await?;
        Ok(())
    }
}
