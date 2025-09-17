use global_utils::config_variant::ConfigVariant;
use global_utils::logger::init_logger;
use spark_balance_checker_config_parser::config::{ServerConfig, get_certificate};
use spark_balance_checker_server::init::create_app;
use spark_client::common::config::SparkConfig;
use tokio::{self, net::TcpListener};

#[tracing::instrument(level = "debug", ret)]
#[tokio::main]
async fn main() {
    let _logger_guard = init_logger();
    let config_path = std::env::var("CONFIG_PATH").unwrap();
    let config = ServerConfig::init_config(ConfigVariant::OnlyOneFilepath(config_path.clone())).unwrap();
    let ca_pem_path = std::env::var("CA_PEM_PATH").unwrap();
    let app = create_app(SparkConfig {
        operators: config.spark_operators,
        ca_pem: get_certificate(ca_pem_path).unwrap(),
    })
    .await;

    let addr_to_listen = format!("{}:{}", config.app_config.ip, config.app_config.port);
    let listener = TcpListener::bind(addr_to_listen.clone()).await.unwrap();

    tracing::info!("Listening on {:?}", addr_to_listen);
    #[cfg(feature = "swagger")]
    {
        tracing::info!("Swagger UI available at {}/swagger-ui/", config.app_config.ip, config.app_config.port);
    }
    axum::serve(listener, app).await.unwrap();
}
