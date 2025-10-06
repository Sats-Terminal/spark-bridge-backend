use frost::aggregator::FrostAggregator;
use gateway_config_parser::config::DkgPregenConfig;
use gateway_local_db_store::schemas::dkg_share::DkgShareGenerate;
use gateway_local_db_store::storage::LocalDbStorage;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tracing::instrument;

static EPOCH: AtomicU64 = AtomicU64::new(0);

#[derive(Clone)]
pub struct DkgPregenThread {
    task_tracker: TaskTracker,
    cancellation_token: CancellationToken,
}

struct UpdatePossibility {
    dkg_available: u64,
    finalized_dkg_available: u64,
}

type Storage = Arc<LocalDbStorage>;
type Aggregator = Arc<FrostAggregator>;

impl DkgPregenThread {
    #[instrument(skip_all, level = "debug", fields(thread = "dkg_pregen_spawning"))]
    pub async fn start(local_db: Storage, dkg_pregen_config: DkgPregenConfig, frost_aggregator: Aggregator) -> Self {
        let cancellation_token = CancellationToken::new();
        let mut task_tracker = TaskTracker::default();
        Self::spawn_thread(
            &mut task_tracker,
            local_db,
            dkg_pregen_config,
            frost_aggregator,
            cancellation_token.clone(),
        )
        .await;
        Self {
            task_tracker,
            cancellation_token,
        }
    }

    #[instrument(skip_all, level = "debug", fields(thread = "dkg_pregen_spawning"))]
    async fn spawn_thread(
        task_tracker: &mut TaskTracker,
        local_db: Storage,
        dkg_pregen_config: DkgPregenConfig,
        frost_aggregator: Aggregator,
        cancellation_token: CancellationToken,
    ) {
        task_tracker.spawn(async move {
            tracing::trace!(dkg_pregen_config = ?dkg_pregen_config, "Loop spawned..");
            let mut interval = tokio::time::interval(Duration::from_millis(dkg_pregen_config.update_interval_millis));
            'checking_loop: loop {
                tokio::select! {
                    _ = cancellation_token.cancelled() => {
                        tracing::trace!("Closing [dkg_pregen] update task, because of cancellation token");
                        break 'checking_loop;
                    },
                    _ = interval.tick() => {
                        Self::perform_update(local_db.clone(), frost_aggregator.clone(), &dkg_pregen_config).await;
                    }
                }
            }
        });
    }

    #[instrument(skip_all, level = "trace", fields(thread = "dkg_pregen"))]
    pub async fn perform_update(local_db: Storage, dkg_aggregator: Aggregator, dkg_pregen_config: &DkgPregenConfig) {
        match Self::get_possible_update_info(local_db.clone()).await {
            Ok(UpdatePossibility {
                dkg_available,
                finalized_dkg_available,
            }) => match Self::get_update_decision(dkg_available, finalized_dkg_available, dkg_pregen_config) {
                0 => {
                    tracing::trace!(
                        "Free dkg values are available: {dkg_available}, \
                                        finalized dkgs: {finalized_dkg_available}, \
                                        not performing update for pregenerated DgkShares"
                    )
                }
                amount_to_gen => {
                    let _ = Self::pregenerate_shares(local_db.clone(), dkg_aggregator.clone(), amount_to_gen)
                        .await
                        .inspect_err(|err| tracing::error!("Failed to pregenerate_shares DgkShares: {err}"));
                }
            },
            Err(err) => {
                tracing::error!("Failed to get possibility_of_update for pregenerated DgkShares: {err}");
            }
        }
    }

    #[instrument(skip_all, level = "trace", fields(thread = "dkg_pregen", ret))]
    async fn get_possible_update_info(local_db: Storage) -> eyre::Result<UpdatePossibility> {
        Ok(UpdatePossibility {
            dkg_available: local_db.count_unused_dkg_shares().await?,
            finalized_dkg_available: local_db.count_unused_finalized_dkg_shares().await?,
        })
    }

    /// Checks availability to generate more dkg pregen values
    #[instrument(level = "trace", fields(thread = "dkg_pregen"))]
    fn get_update_decision(_dkg_available: u64, finalized_dkg_available: u64, config: &DkgPregenConfig) -> u64 {
        config.min_threshold.saturating_sub(finalized_dkg_available)
    }

    /// Pregenerates shares for dkg state
    #[instrument(skip(local_db, dkg_aggregator), level = "debug", fields(path = "dkg_pregen_thread"))]
    async fn pregenerate_shares(local_db: Storage, dkg_aggregator: Aggregator, amount: u64) -> eyre::Result<()> {
        let mut join_set = JoinSet::new();
        tracing::trace!("Pregenerating epoch {}", EPOCH.load(Ordering::SeqCst));
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

    #[instrument(level = "trace", skip(local_db, aggregator), fields(epoch=EPOCH.load(Ordering::SeqCst)
    ), err)]
    async fn pregenerate_share(local_db: Storage, aggregator: Aggregator) -> eyre::Result<()> {
        let initialized_entity = local_db.generate_dkg_share_entity().await?;
        aggregator.run_dkg_flow(&initialized_entity).await?;
        Ok(())
    }
}

impl Drop for DkgPregenThread {
    fn drop(&mut self) {
        self.cancellation_token.cancel();
        self.task_tracker.close();
    }
}
