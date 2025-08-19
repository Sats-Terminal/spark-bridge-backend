use bridge_rest_api::startup::run_server;
use config_parser::config::{ServerConfig, get_app_config_val};
use global_utils::logger::init_logger;

#[tokio::main]
async fn main() -> bridge_rest_api::error::Result<()> {
    let _logger_guard = init_logger();
    let config = ServerConfig::init_config(get_app_config_val())?;
    let server_addr = config.app_config.get_app_binding_url()?;
    run_server(server_addr, config).await?.await?;
    Ok(())
}
