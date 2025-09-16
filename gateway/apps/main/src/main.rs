use anyhow::{anyhow, bail};
use gateway_config_parser::config::ServerConfig;
use gateway_flow_processor::init::create_flow_processor;
use gateway_local_db_store::storage::LocalDbStorage;
use global_utils::config_path::ConfigPath;
use global_utils::config_variant::ConfigVariant;
use global_utils::env_parser::lookup_ip_addr;
use global_utils::logger::init_logger;
use global_utils::network::NetworkConfig;
use persistent_storage::config::PostgresDbCredentials;
use persistent_storage::init::PostgresRepo;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::instrument;

#[instrument(level = "trace", ret)]
#[tokio::main]
async fn main() {
    // let _ = dotenv::dotenv();
    // let _logger_guard = init_logger();

    // let config_path = ConfigPath::from_env()?;
    // let network_config = NetworkConfig::from_env()?;
    // let app_config = ServerConfig::init_config(ConfigVariant::OnlyOneFilepath(config_path.path))?;
    // tracing::debug!("App config: {:?}", app_config);

    // let postgres_creds = PostgresDbCredentials::from_db_url()?;
    // let db_pool = LocalDbStorage {
    //     postgres_repo: PostgresRepo::from_config(postgres_creds).await?,
    // };
    // let shared_db_pool = Arc::new(db_pool);

    // // Aggregators creation
    // let frost_aggregator =
    //     create_aggregator_from_config(app_config.clone(), shared_db_pool.clone(), shared_db_pool.clone())?;
    // let btc_resp_checker_aggregator = Arc::new(create_btc_resp_checker_aggregator_from_config(app_config.clone())?);

    // let (mut flow_processor, flow_sender) = create_flow_processor(
    //     shared_db_pool,
    //     app_config.flow_processor.cancellation_retries,
    //     frost_aggregator,
    //     network_config.network,
    // );

    // let _ = tokio::spawn(async move {
    //     flow_processor.run().await;
    // });

    // let private_app =
    //     gateway_server::init::create_private_app(flow_sender.clone(), btc_resp_checker_aggregator.clone()).await?;
    // let addr_to_listen_private = (
    //     lookup_ip_addr(&app_config.server_private_api.ip)?,
    //     app_config.server_private_api.port,
    // );
    // let listener_private = TcpListener::bind(addr_to_listen_private.clone())
    //     .await
    //     .map_err(|e| anyhow!("Failed to bind to private address: {}", e))?;
    // let private_app_server = axum::serve(listener_private, private_app).into_future();

    // let addr_to_listen_public = (
    //     lookup_ip_addr(&app_config.server_public.ip)?,
    //     app_config.server_public.port,
    // );
    // let public_app = gateway_server::init::create_public_app(
    //     flow_sender.clone(),
    //     btc_resp_checker_aggregator.clone(),
    //     addr_to_listen_public,
    // )
    // .await?;
    // let addr_to_listen_public = (
    //     lookup_ip_addr(&app_config.server_public.ip)?,
    //     app_config.server_public.port,
    // );
    // let listener_public = TcpListener::bind(addr_to_listen_public)
    //     .await
    //     .map_err(|e| anyhow!("Failed to bind to public address: {}", e))?;
    // let public_app_server = axum::serve(listener_public, public_app).into_future();

    // let (public_res, private_res) = futures::join!(public_app_server, private_app_server);
    // match public_res {
    //     Ok(_) => match private_res {
    //         Ok(_) => Ok(()),
    //         Err(e_private) => {
    //             bail!("Failed to serve private server: {}", e_private)
    //         }
    //     },
    //     Err(e_public) => match private_res {
    //         Ok(_) => Ok(()),
    //         Err(e_private) => {
    //             bail!(
    //                 "Failed to serve private server: {} & public server: {}",
    //                 e_private,
    //                 e_public
    //             )
    //         }
    //     },
    // }
}
