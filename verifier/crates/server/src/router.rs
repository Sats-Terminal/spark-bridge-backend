use axum::{
    routing::{
        Router,
        post,
    }
};
use crate::handlers::{
    handler_watch_spark_address,
    handler_watch_runes_address,
};

pub fn create_router() -> Router {
    Router::new()
        .route("/api/verifier/watch-spark-address", post(handler_watch_spark_address))
        .route("/api/verifier/watch-runes-address", post(handler_watch_runes_address))
}