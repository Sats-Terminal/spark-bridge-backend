use std::{net::SocketAddr, str::FromStr};

use btc_indexer_server::{
    common::SocketAddrWrapped,
    routes::{track_tx::TrackTxRequest, track_wallet::TrackWalletRequest},
};
use tracing::info;

use crate::utils::init::{TEST_LOGGER, init_test_server, obtain_random_addr};

mod utils;

#[tokio::test]
async fn test_invocation_wallet_tracking() -> anyhow::Result<()> {
    dotenv::dotenv()?;
    let _logger_guard = &*TEST_LOGGER;
    let test_server = init_test_server().await?;
    let response = test_server
        .post("/track_wallet")
        .json(&TrackWalletRequest {
            wallet: "bc1qvvwhefadjpsnynen8e4n2g3tc3d3hvtraemaxw".to_string(),
            callback_url: SocketAddrWrapped(obtain_random_addr()?),
        })
        .await;
    info!("response: {:?}", response);
    Ok(())
}
