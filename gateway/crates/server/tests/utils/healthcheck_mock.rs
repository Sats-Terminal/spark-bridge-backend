use crate::utils::common::{CERT_PATH, CONFIG_PATH, obtain_random_localhost_socket_addr};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router, debug_handler};
use axum_test::TestServer;
use frost::aggregator::FrostAggregator;
use frost::signer::FrostSigner;
use frost::traits::SignerClient;
use frost_secp256k1_tr::Identifier;
use gateway_config_parser::config::ServerConfig;
use gateway_deposit_verification::aggregator::DepositVerificationAggregator;
use gateway_deposit_verification::traits::VerificationClient;
use gateway_flow_processor::init::create_flow_processor;
use gateway_local_db_store::storage::LocalDbStorage;
use gateway_server::init::{GatewayApi, create_app};
use gateway_verifier_client::client::VerifierClient;
use global_utils::common_resp::Empty;
use global_utils::config_path::ConfigPath;
use global_utils::logger::init_logger;
use persistent_storage::config::PostgresDbCredentials;
use persistent_storage::init::{PostgresPool, PostgresRepo};
use std::collections::{BTreeMap, HashMap};
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio_util::task::{TaskTracker, task_tracker};
use tracing::{info, instrument};
use url::Url;
use verifier_server::init::VerifierApi;

#[instrument(skip(pool))]
pub async fn init_mocked_test_server(pool: PostgresPool) -> anyhow::Result<TestServer> {
    let config_path = ConfigPath {
        path: CONFIG_PATH.to_string(),
    };
    let mut server_config = ServerConfig::init_config(config_path.path);
    tracing::debug!("App config: {:?}", server_config);

    server_config.spark.certificate.path = CERT_PATH.to_string();

    // Create DB Pool
    let db_pool = Arc::new(LocalDbStorage {
        postgres_repo: PostgresRepo { pool },
    });

    // Spawn mocked verifiers
    for v_conf in server_config.verifiers.0.iter_mut() {
        let addr_to_listen = obtain_random_localhost_socket_addr()?;
        v_conf.address = format!("http://{}", addr_to_listen);

        let mock_health_app = create_mock_healthcheck_app();
        info!("Addr to send: {:?}", v_conf.address);
        let listener = TcpListener::bind(addr_to_listen).await?;
        tokio::spawn(async move {
            axum::serve(listener, mock_health_app).await.unwrap();
        });
    }

    // Create Frost Aggregator
    let mut verifiers_map = BTreeMap::<Identifier, Arc<dyn SignerClient>>::new();
    for verifier in server_config.clone().verifiers.0 {
        let identifier: Identifier = verifier.id.try_into()?;
        let verifier_client = VerifierClient::new(verifier);
        verifiers_map.insert(identifier, Arc::new(verifier_client));
    }
    let frost_aggregator = FrostAggregator::new(verifiers_map, db_pool.clone(), db_pool.clone());

    // Create Flow Processor
    let (mut flow_processor, flow_sender) = create_flow_processor(
        server_config.clone(),
        db_pool.clone(),
        server_config.flow_processor.cancellation_retries,
        frost_aggregator,
        server_config.network.network,
    )
    .await;

    let mut task_tracker = TaskTracker::default();
    task_tracker.spawn(async move {
        flow_processor.run().await;
    });

    // Create Deposit Verification Aggregator
    let (verifier_clients_hash_map, typed_verifier_clients_hash_map) = extract_verifiers(&server_config);
    let deposit_verification_aggregator =
        DepositVerificationAggregator::new(flow_sender.clone(), verifier_clients_hash_map, db_pool.clone());

    // Create App
    let app = create_app(
        flow_sender.clone(),
        deposit_verification_aggregator.clone(),
        server_config.network.network,
        typed_verifier_clients_hash_map,
        db_pool,
        task_tracker,
    )
    .await;

    // Run App
    let addr_to_listen = format!(
        "{}:{}",
        server_config.server_public.ip, server_config.server_public.port
    );
    let listener = TcpListener::bind(addr_to_listen.clone()).await?;
    tracing::info!("Listening on {:?}", addr_to_listen);

    let test_server = TestServer::builder().http_transport().build(app.into_make_service())?;
    info!("Serving local axum test server on {:?}", test_server.server_address());
    Ok(test_server)
}

fn extract_verifiers(
    server_config: &ServerConfig,
) -> (
    HashMap<u16, Arc<dyn VerificationClient>>,
    HashMap<u16, Arc<VerifierClient>>,
) {
    let mut verifier_clients_hash_map = HashMap::<u16, Arc<dyn VerificationClient>>::new();
    let mut typed_verifier_clients_hash_map = HashMap::<u16, Arc<VerifierClient>>::new();
    for verifier in server_config.clone().verifiers.0 {
        let verifier_client = VerifierClient::new(verifier.clone());
        verifier_clients_hash_map.insert(verifier.id, Arc::new(verifier_client.clone()));
        typed_verifier_clients_hash_map.insert(verifier.id, Arc::new(verifier_client));
    }
    (verifier_clients_hash_map, typed_verifier_clients_hash_map)
}

fn create_mock_healthcheck_app() -> Router {
    Router::new().route(VerifierApi::HEALTHCHECK_ENDPOINT, post(handle_healthcheck))
}

#[derive(thiserror::Error, Debug)]
enum DraftResult {}

impl IntoResponse for DraftResult {
    fn into_response(self) -> Response {
        (StatusCode::BAD_REQUEST, "msg".to_string()).into_response()
    }
}

#[debug_handler]
async fn handle_healthcheck() -> Result<Json<Empty>, DraftResult> {
    Ok((Json(Empty {})))
}
