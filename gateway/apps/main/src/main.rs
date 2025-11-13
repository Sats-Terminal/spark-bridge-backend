use std::{
    collections::{BTreeMap, HashMap},
    sync::{Arc, Once},
};

use eyre::Result;
use frost::{aggregator::FrostAggregator, traits::SignerClient};
use frost_secp256k1_tr::Identifier;
use gateway_config_parser::config::ServerConfig;
use gateway_deposit_verification::{aggregator::DepositVerificationAggregator, traits::VerificationClient};
use gateway_dkg_pregen::dkg_pregen_thread::DkgPregenThread;
use gateway_flow_processor::init::create_flow_processor;
use gateway_local_db_store::storage::LocalDbStorage;
use gateway_server::init::create_app;
use gateway_verifier_client::client::VerifierClient;
use global_utils::{config_path::ConfigPath, logger::init_logger};
use persistent_storage::{config::PostgresDbCredentials, init::PostgresRepo};
use tokio::net::TcpListener;
use tokio_util::task::TaskTracker;
use tracing::instrument;

fn install_rustls_provider() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        rustls::crypto::ring::default_provider()
            .install_default()
            .expect("install rustls crypto provider");
    });
}

#[instrument(level = "trace", ret)]
#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenvy::dotenv();
    let _logger_guard = init_logger();

    install_rustls_provider();

    // Create Config
    let config_path = ConfigPath::from_env().map_err(|e| eyre::eyre!("Failed to parse config path: {}", e))?;
    let server_config = ServerConfig::init_config(config_path.path);
    tracing::debug!("App config: {:?}", server_config);

    // Create DB Pool
    let postgres_creds = PostgresDbCredentials {
        url: server_config.database.url.clone(),
    };
    let db_pool = LocalDbStorage {
        postgres_repo: PostgresRepo::from_config(postgres_creds)
            .await
            .map_err(|e| eyre::eyre!("Failed to create DB pool: {}", e))?,
        network: server_config.network.network,
    };
    let shared_db_pool = Arc::new(db_pool);

    // Create Frost Aggregator
    let mut verifiers_map = BTreeMap::<Identifier, Arc<dyn SignerClient>>::new();
    for verifier in server_config.clone().verifiers.0 {
        let identifier: Identifier = verifier
            .id
            .try_into()
            .map_err(|e| eyre::eyre!("Failed to parse identifier: {}", e))?;
        let verifier_client = VerifierClient::new(verifier);
        verifiers_map.insert(identifier, Arc::new(verifier_client));
    }
    let frost_aggregator = Arc::new(FrostAggregator::new(
        verifiers_map,
        shared_db_pool.clone(),
        shared_db_pool.clone(),
    ));

    // Create Flow Processor
    let (mut flow_processor, flow_sender) = create_flow_processor(
        server_config.clone(),
        shared_db_pool.clone(),
        server_config.flow_processor.cancellation_retries,
        frost_aggregator.clone(),
        server_config.network.network,
    )
    .await?;

    let task_tracker = TaskTracker::default();
    task_tracker.spawn(async move {
        flow_processor.run().await;
    });

    let dkg_pregen = DkgPregenThread::start(
        shared_db_pool.clone(),
        server_config.dkg_pregen_config,
        frost_aggregator.clone(),
    )
    .await;

    // Create Deposit Verification Aggregator
    let verifier_clients_hash_map = extract_verification_clients(&server_config);
    let deposit_verification_aggregator = DepositVerificationAggregator::new(
        flow_sender.clone(),
        verifier_clients_hash_map,
        shared_db_pool.clone(),
        server_config.network.network,
    );

    // Create App
    let app = create_app(
        flow_sender.clone(),
        deposit_verification_aggregator.clone(),
        server_config.network.network,
        task_tracker,
        dkg_pregen,
        server_config.verifiers,
        server_config.fee,
    )
    .await;

    // Run App
    let addr_to_listen = format!(
        "{}:{}",
        server_config.server_public.ip, server_config.server_public.port
    );
    let listener = TcpListener::bind(addr_to_listen.clone())
        .await
        .map_err(|e| eyre::eyre!("Failed to bind listener: {}", e))?;
    tracing::info!("Listening on {:?}", addr_to_listen);
    axum::serve(listener, app)
        .await
        .map_err(|e| eyre::eyre!("Failed to serve app: {}", e))?;

    Ok(())
}

fn extract_verification_clients(server_config: &ServerConfig) -> HashMap<u16, Arc<dyn VerificationClient>> {
    let mut verifier_clients_hash_map = HashMap::<u16, Arc<dyn VerificationClient>>::new();
    for verifier in &server_config.verifiers.0 {
        let verifier_client = VerifierClient::new(verifier.clone());
        verifier_clients_hash_map.insert(verifier.id, Arc::new(verifier_client.clone()));
    }
    verifier_clients_hash_map
}
