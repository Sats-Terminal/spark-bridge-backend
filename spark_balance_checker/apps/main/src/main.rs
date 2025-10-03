use eyre::Result;
use global_utils::config_path::ConfigPath;
use global_utils::logger::init_logger;
use spark_balance_checker_config_parser::config::ServerConfig;
use spark_balance_checker_server::init::create_app;
use std::sync::Once;
use tokio::{self, net::TcpListener};

fn install_rustls_provider() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        rustls::crypto::ring::default_provider()
            .install_default()
            .expect("install rustls crypto provider");
    });
}

#[tracing::instrument(level = "debug", ret)]
#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenvy::dotenv();
    let _logger_guard = init_logger();
    install_rustls_provider();

    // Init configs
    let config_path = ConfigPath::from_env().map_err(|e| eyre::eyre!("Failed to parse config path: {}", e))?;
    let config = ServerConfig::init_config(config_path.path);
    let app = create_app(config.spark.clone())
        .await
        .map_err(|e| eyre::eyre!("Failed to create app: {}", e))?;

    // Init app
    let addr_to_listen = format!("{}:{}", config.app_config.ip, config.app_config.port);
    let listener = TcpListener::bind(addr_to_listen.clone())
        .await
        .map_err(|e| eyre::eyre!("Failed to bind listener: {}", e))?;

    tracing::info!("Listening on {:?}", addr_to_listen);
    #[cfg(feature = "swagger")]
    {
        tracing::info!(
            "Swagger UI available at {}/swagger-ui/",
            config.app_config.ip,
            config.app_config.port
        );
    }
    axum::serve(listener, app)
        .await
        .map_err(|e| eyre::eyre!("Failed to serve app: {}", e))?;

    Ok(())
}
