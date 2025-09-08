use anyhow::anyhow;
use gateway_config_parser::config::ServerConfig;
use gateway_flow_processor::init::create_flow_processor;
use gateway_local_db_store::storage::LocalDbStorage;
use gateway_utils::aggregator::create_aggregator_from_config;
use global_utils::config_path::ConfigPath;
use global_utils::config_variant::ConfigVariant;
use global_utils::env_parser::lookup_ip_addr;
use global_utils::logger::init_logger;
use persistent_storage::config::PostgresDbCredentials;
use persistent_storage::init::PostgresRepo;
use tokio::net::TcpListener;
use tracing::instrument;

#[instrument(level = "debug", ret)]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _logger_guard = init_logger();
    let _ = dotenv::dotenv();

    let config_path = ConfigPath::from_env()?;

    let app_config = ServerConfig::init_config(ConfigVariant::OnlyOneFilepath(config_path.path))?;
    tracing::debug!("App config: {:?}", app_config);

    let frost_aggregator = create_aggregator_from_config(app_config.clone());

    let postgres_creds = PostgresDbCredentials::from_db_url()?;
    let db_pool = LocalDbStorage {
        postgres_repo: PostgresRepo::from_config(postgres_creds).await?,
    };

    let (mut flow_processor, flow_sender) = create_flow_processor(db_pool, 10, frost_aggregator);

    let _ = tokio::spawn(async move {
        flow_processor.run().await;
    });

    let app = gateway_server::init::create_app(flow_sender).await?;

    let addr_to_listen = (lookup_ip_addr(&app_config.server.ip)?, app_config.server.port);
    let listener = TcpListener::bind(addr_to_listen)
        .await
        .map_err(|e| anyhow!("Failed to bind to address: {}", e))?;
    axum::serve(listener, app)
        .await
        .map_err(|e| anyhow!("Failed to serve: {}", e))
}
