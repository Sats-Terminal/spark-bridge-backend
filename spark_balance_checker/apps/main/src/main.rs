use global_utils::config_variant::ConfigVariant;
use global_utils::env_parser::lookup_ip_addr;
use global_utils::logger::init_logger;
use spark_balance_checker_config_parser::config::{ServerConfig, obtain_tonic_ca_cert};
use spark_balance_checker_server::init::create_app;
use spark_client::SparkConfig;
use tokio::{self, net::TcpListener};

#[tracing::instrument(level = "debug", ret)]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _logger_guard = init_logger();
    let config_variant = ConfigVariant::init();
    let config = ServerConfig::init_config(config_variant)?;
    let app = create_app(SparkConfig {
        operators: config.spark_operators,
        ca_pem: obtain_tonic_ca_cert(config_variant)?,
    })
    .await;

    let addr_to_listen = (lookup_ip_addr(&config.app_config.ip)?, config.app_config.port);
    let listener = TcpListener::bind(addr_to_listen).await?;

    tracing::info!("Listening on {:?}", addr_to_listen);
    #[cfg(feature = "swagger")]
    {
        tracing::info!("Swagger UI available at {}/swagger-ui/", config.server.address);
    }
    Ok(axum::serve(listener, app).await?)
}
