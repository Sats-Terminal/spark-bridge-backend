use axum::routing::{Router, post};

use crate::handlers::{
    handler_bridge_runes, handler_exit_spark, handler_get_runes_address, handler_notify_runes_address,
    handler_notify_spark_address,
};

pub fn create_router() -> Router {
    Router::new()
        .route("/api/user/runes-address", post(handler_get_runes_address))
        .route("/api/user/bridge-runes", post(handler_bridge_runes))
        .route("/api/user/exit-spark", post(handler_exit_spark))
        .route("/api/verifier/notify-runes-address", post(handler_notify_runes_address))
        .route("/api/verifier/notify-spark-address", post(handler_notify_spark_address))
}
