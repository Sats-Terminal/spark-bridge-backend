use frost::aggregator::FrostAggregator;
use frost::traits::SignerClient;
use frost_secp256k1_tr::Identifier;
use gateway_config_parser::config::ServerConfig;
use gateway_deposit_verification::aggregator::DepositVerificationAggregator;
use gateway_deposit_verification::traits::VerificationClient;
use gateway_flow_processor::init::create_flow_processor;
use gateway_local_db_store::storage::LocalDbStorage;
use gateway_server::init::create_app;
use gateway_verifier_client::client::VerifierClient;
use global_utils::config_path::ConfigPath;
use global_utils::config_variant::ConfigVariant;
use global_utils::logger::init_logger;
use global_utils::network::NetworkConfig;
use persistent_storage::config::PostgresDbCredentials;
use persistent_storage::init::PostgresRepo;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::instrument;

#[instrument(level = "trace", ret)]
#[tokio::main]
async fn main() {
    let _ = dotenv::dotenv();
    let _logger_guard = init_logger();

    // Create Config
    let config_path = ConfigPath::from_env().unwrap();
    let server_config = ServerConfig::init_config(config_path.path);
    tracing::debug!("App config: {:?}", server_config);

    // Create DB Pool
    let postgres_creds = PostgresDbCredentials {
        url: server_config.database.url.clone(),
    };
    let db_pool = LocalDbStorage {
        postgres_repo: PostgresRepo::from_config(postgres_creds).await.unwrap(),
    };
    let shared_db_pool = Arc::new(db_pool);

    // Create Frost Aggregator
    let mut verifiers_map = BTreeMap::<Identifier, Arc<dyn SignerClient>>::new();
    for verifier in server_config.clone().verifiers.0 {
        let identifier: Identifier = verifier.id.try_into().unwrap();
        let verifier_client = VerifierClient::new(verifier);
        verifiers_map.insert(identifier, Arc::new(verifier_client));
    }
    let frost_aggregator = FrostAggregator::new(verifiers_map, shared_db_pool.clone(), shared_db_pool.clone());

    // Create Flow Processor
    let (mut flow_processor, flow_sender) = create_flow_processor(
        server_config.clone(),
        shared_db_pool.clone(),
        server_config.flow_processor.cancellation_retries,
        frost_aggregator,
        server_config.network.network,
    ).await;
    let _ = tokio::spawn(async move {
        flow_processor.run().await;
    });

    // Create Deposit Verification Aggregator
    let mut verifier_clients_hash_map = HashMap::<u16, Arc<dyn VerificationClient>>::new();
    for verifier in server_config.clone().verifiers.0 {
        let verifier_client = VerifierClient::new(verifier.clone());
        verifier_clients_hash_map.insert(verifier.id, Arc::new(verifier_client));
    }
    let deposit_verification_aggregator =
        DepositVerificationAggregator::new(flow_sender.clone(), verifier_clients_hash_map, shared_db_pool.clone());

    // Create App
    let app = create_app(
        flow_sender.clone(),
        deposit_verification_aggregator.clone(),
        server_config.network.network,
    )
    .await;

    // Run App
    let addr_to_listen = format!(
        "{}:{}",
        server_config.server_public.ip, server_config.server_public.port
    );
    let listener = TcpListener::bind(addr_to_listen).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
