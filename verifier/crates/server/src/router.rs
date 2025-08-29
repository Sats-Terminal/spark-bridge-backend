use axum::{
    routing::{
        Router,
        post,
    }
};
use crate::handlers::{
    handler_watch_spark_address,
    handler_watch_runes_address,
    handler_get_public_key_package,
};

pub fn create_router() -> Router {
    Router::new()
        .route("/api/verifier/watch-spark-address", post(handler_watch_spark_address))
        .route("/api/verifier/watch-runes-address", post(handler_watch_runes_address))
        .route("/api/verifier/get-public-key-package", post(handler_get_public_key_package))
}