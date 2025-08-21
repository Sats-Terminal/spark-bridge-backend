use std::{net::SocketAddr, str::FromStr};

use btc_indexer_server::{common::SocketAddrWrapped, routes::track_tx::TrackTxRequest};
use tracing::info;

use crate::utils::init::{TEST_LOGGER, init_test_server, obtain_random_addr};

mod utils;

#[tokio::test]
async fn test_invocation_tx_tracking() -> anyhow::Result<()> {
    dotenv::dotenv()?;
    let _logger_guard = &*TEST_LOGGER;
    let test_server = init_test_server().await?;
    let response = test_server
        .post("/track_tx")
        .json(&TrackTxRequest {
            tx_id: "fb0c9ab881331ec7acdd85d79e3197dcaf3f95055af1703aeee87e0d853e81ec".to_string(),
            callback_url: SocketAddrWrapped(obtain_random_addr()?),
        })
        .await;
    info!("response: {:?}", response);
    Ok(())
}
