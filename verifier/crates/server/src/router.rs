use axum::{
    routing::{
        Router,
        post,
    }
};
use crate::handlers::{
    watchers::handler_watch_spark_address,
    watchers::handler_watch_runes_address,
    address::handler_get_public_key_package,
    dkg::handler_get_round_1_package,
    dkg::handler_get_round_2_package,
    dkg::handler_get_round_3_package,
};

pub fn create_router() -> Router {
    Router::new()
        .route("/api/gateway/watch-spark-address", post(handler_watch_spark_address))
        .route("/api/gateway/watch-runes-address", post(handler_watch_runes_address))
        .route("/api/gateway/get-public-key-package", post(handler_get_public_key_package))
        .route("/api/gateway/get-round-1-package", post(handler_get_round_1_package))
        .route("/api/gateway/get-round-2-package", post(handler_get_round_2_package))
        .route("/api/gateway/get-round-3-package", post(handler_get_round_3_package))
}