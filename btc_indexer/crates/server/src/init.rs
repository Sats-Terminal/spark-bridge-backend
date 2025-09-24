use std::sync::Arc;

use axum::{Router, routing::post};
use btc_indexer_api::api::BtcIndexerApi;
use btc_indexer_internals::indexer::BtcIndexer;
use btc_indexer_internals::tx_arbiter::TxArbiter;
use local_db_store_indexer::init::LocalDbStorage;
use titan_client::TitanClient;
use tracing::instrument;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

#[derive(Clone)]
pub struct AppState<C, Db, TxValidator> {
    pub http_client: reqwest::Client,
    pub persistent_storage: Arc<Db>,
    pub btc_indexer: Arc<BtcIndexer<C, Db, TxValidator>>,
}

#[derive(OpenApi)]
#[openapi(paths(crate::routes::track_tx::handler))]
struct ApiDoc;

#[instrument(skip(db_pool, btc_indexer))]
pub async fn create_app(
    db_pool: LocalDbStorage,
    btc_indexer: BtcIndexer<TitanClient, LocalDbStorage, TxArbiter>,
) -> Router {
    // We're opening already tracking task for our txs
    let (btc_indexer, db_pool) = (Arc::new(btc_indexer), Arc::new(db_pool));

    let state = AppState {
        http_client: reqwest::Client::new(),
        persistent_storage: db_pool,
        btc_indexer,
    };
    let app = Router::new()
        .route(BtcIndexerApi::TRACK_TX_ENDPOINT, post(crate::routes::track_tx::handler))
        .route(
            BtcIndexerApi::HEALTHCHECK_ENDPOINT,
            post(crate::routes::healthcheck::handler),
        )
        .with_state(state);

    #[cfg(feature = "swagger")]
    let app = app.merge(SwaggerUi::new("/swagger-ui/").url("/api-docs/openapi.json", ApiDoc::openapi()));
    app
}
