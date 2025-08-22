use std::{collections::HashMap, sync::Arc};

use axum::{Router, routing::post};
use btc_indexer_internals::indexer::BtcIndexer;
use persistent_storage::init::PersistentRepoShared;
use titan_client::TitanClient;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tracing::instrument;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;
use uuid::Uuid;

pub type CachedTasks = Arc<RwLock<HashMap<Uuid, CancellationToken>>>;

#[derive(Clone)]
pub struct AppState<C> {
    pub http_client: reqwest::Client,
    pub persistent_storage: PersistentRepoShared,
    pub btc_indexer: Arc<BtcIndexer<C>>,
    pub cached_tasks: CachedTasks,
}

#[derive(OpenApi)]
#[openapi(paths(crate::routes::track_tx::handler, crate::routes::track_wallet::handler))]
struct ApiDoc;

#[instrument(skip(db_pool, btc_indexer))]
pub async fn create_app(db_pool: PersistentRepoShared, btc_indexer: BtcIndexer<TitanClient>) -> Router {
    let state = AppState {
        http_client: reqwest::Client::new(),
        persistent_storage: db_pool,
        btc_indexer: Arc::new(btc_indexer),
        cached_tasks: Arc::new(Default::default()),
    };
    let app = Router::new()
        .route("/track_tx", post(crate::routes::track_tx::handler))
        .route("/track_wallet", post(crate::routes::track_wallet::handler))
        .with_state(state);

    #[cfg(feature = "swagger")]
    let app = app.merge(SwaggerUi::new("/swagger-ui/").url("/api-docs/openapi.json", ApiDoc::openapi()));

    app
}
