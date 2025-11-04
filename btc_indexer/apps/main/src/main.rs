use anyhow::anyhow;
use bitcoin::Network;
use btc_indexer_internals::indexer::{BtcIndexer, IndexerParams};
use config_parser::config::{BtcRpcCredentials, MaestroConfig, ServerConfig, TitanConfig};
use global_utils::config_path::ConfigPath;
use global_utils::config_variant::ConfigVariant;
use global_utils::{env_parser::lookup_ip_addr, logger::init_logger};
use local_db_store_indexer::{PostgresDbCredentials, init::LocalDbStorage};

use tokio::net::TcpListener;
use tracing::instrument;

#[instrument(level = "debug", ret)]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();
    let _logger_guard = init_logger();

    // Init configs
    let config_path = ConfigPath::from_env()?;
    let app_config = ServerConfig::init_config(ConfigVariant::OnlyOneFilepath(config_path.path))?;

    let btc_creds = BtcRpcCredentials::new()?;
    let postgres_creds = PostgresDbCredentials::from_db_url()?;
    let titan_config = TitanConfig::maybe_new()?;
    let maestro_config = MaestroConfig::maybe_from_env()?;

    match btc_creds.network {
        Network::Regtest => {
            if titan_config.is_none() {
                return Err(anyhow!("TITAN_URL must be provided when running on regtest"));
            }
        }
        _ => {
            if maestro_config.is_none() {
                return Err(anyhow!(
                    "MAESTRO_API_URL and MAESTRO_API_KEY must be provided when BITCOIN_NETWORK is not regtest"
                ));
            }
        }
    };

    // Init App
    let db_pool = LocalDbStorage::from_config(postgres_creds).await?;
    let btc_indexer = BtcIndexer::with_api(IndexerParams {
        titan_config,
        maestro_config,
        btc_rpc_creds: btc_creds,
        db_pool: db_pool.clone(),
        btc_indexer_params: app_config.btc_indexer_config,
    })?;
    let app = btc_indexer_server::create_app(db_pool, btc_indexer).await;

    let addr_to_listen = (
        lookup_ip_addr(&app_config.app_config.http_server_ip)?,
        app_config.app_config.http_server_port,
    );
    let listener = TcpListener::bind(addr_to_listen).await?;

    tracing::info!("Listening on {:?}", addr_to_listen);
    #[cfg(feature = "swagger")]
    tracing::info!("Swagger UI available at {:?}/swagger-ui/", addr_to_listen);

    Ok(axum::serve(listener, app).await?)
}
