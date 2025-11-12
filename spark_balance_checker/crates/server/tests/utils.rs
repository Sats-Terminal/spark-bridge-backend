use axum::ServiceExt;
use axum_test::TestServer;
use eyre::eyre;
use global_utils::config_path::ConfigPath;
use global_utils::config_variant::ConfigVariant;
use global_utils::logger::{LoggerGuard, init_logger};
use spark_balance_checker_config_parser::config::ServerConfig;
use spark_balance_checker_server::init::create_app;
use spark_client::common::config::{CertificateConfig, SparkConfig};
use std::sync::{Arc, LazyLock};
use tracing::{info, instrument};

pub static TEST_LOGGER: LazyLock<LoggerGuard> = LazyLock::new(|| init_logger());
const CONFIG_PATH: &str = "../../../infrastructure/configurations/spark_balance_checker/dev.toml";
const PATH_TO_AMAZON_CA: &str = "../../../infrastructure/configurations/certificates/Amazon-Root-CA.pem";
const PATH_TO_FLASHNET: &str = "../../../infrastructure/configurations/certificates/Flashnet-CA.pem";

#[instrument(ret)]
pub async fn init_mocked_test_server() -> eyre::Result<TestServer> {
    let config_path = ConfigPath {
        path: CONFIG_PATH.to_string(),
    };
    let mut config = ServerConfig::init_config(config_path.path);
    config.spark.certificates = vec![
        CertificateConfig {
            path: PATH_TO_AMAZON_CA.to_string(),
        },
        CertificateConfig {
            path: PATH_TO_FLASHNET.to_string(),
        },
    ];

    let app = create_app(config).await?;
    let test_server = TestServer::builder()
        .http_transport()
        .build(app.into_make_service())
        .map_err(|err| eyre!(Box::new(err)))?;
    info!("Serving local axum test server on {:?}", test_server.server_address());
    Ok(test_server)
}
