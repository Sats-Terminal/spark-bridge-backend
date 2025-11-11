use axum::response::Json;

pub async fn handle() -> Json<()> {
    Json(())
}
