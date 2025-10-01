use axum_test::TestServer;
use global_utils::config_path::ConfigPath;
use global_utils::config_variant::ConfigVariant;
use global_utils::logger::{LoggerGuard, init_logger};
use spark_balance_checker_config_parser::config::ServerConfig;
use spark_balance_checker_server::init::create_app;
use spark_client::common::config::SparkConfig;
use std::sync::{Arc, LazyLock};
use tracing::{info, instrument};

pub static TEST_LOGGER: LazyLock<LoggerGuard> = LazyLock::new(|| init_logger());
const CONFIG_PATH: &str = "../../../infrastructure/configurations/spark_balance_checker/dev.toml";
const CERT_PATH: &str = "../../../infrastructure/configurations/common/ca.pem";
const CERT_1_PATH: &str = "../../../infrastructure/configurations/certificates/Amazon-Root-CA.pem";
const CERT_2_PATH: &str = "../../../infrastructure/configurations/certificates/Flashnet-CA.pem";

#[instrument(ret)]
pub async fn init_mocked_test_server() -> anyhow::Result<TestServer> {
    let config_path = ConfigPath {
        path: CONFIG_PATH.to_string(),
    };
    let mut config = ServerConfig::init_config(config_path.path);
    config.spark.certificates[0].path = CERT_1_PATH.to_string();
    config.spark.certificates[1].path = CERT_2_PATH.to_string();

    let app = create_app(config.spark).await;
    let test_server = TestServer::builder().http_transport().build(app.into_make_service())?;
    info!("Serving local axum test server on {:?}", test_server.server_address());
    Ok(test_server)
}
