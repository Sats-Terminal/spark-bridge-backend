use axum::{
    body::Body,
    extract::{Request, State},
    http::{Response, StatusCode},
    middleware::Next,
    response::IntoResponse,
};
use bitcoin::{
    hashes::{Hash, HashEngine, sha256::Hash as SHA256},
    key::Secp256k1,
    secp256k1::Message,
};
use tracing::{debug, error};

use crate::init::AppState;

pub async fn build_signature(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let res = next.run(req).await;

    let Some(secret_key) = state.server_config.server.secret_key else {
        return Ok(res);
    };

    let (mut parts, body) = res.into_parts();

    let bytes = match axum::body::to_bytes(body, usize::MAX).await {
        Ok(collected) => collected,
        Err(e) => {
            error!("failed to read response body: {}", e);
            return Ok((StatusCode::INTERNAL_SERVER_ERROR, "Internal server error").into_response());
        }
    };
    debug!("Generating response signature");

    let mut hasher = SHA256::engine();
    hasher.input(&bytes);
    let hash = SHA256::from_engine(hasher);
    let msg = Message::from_digest(hash.to_byte_array());

    let secp = Secp256k1::new();
    let signature = secp.sign_ecdsa(&msg, &secret_key).serialize_der();

    debug!("Response signature generated successfully");

    parts
        .headers
        .append("x-signature", signature.to_string().parse().unwrap());

    Ok(Response::from_parts(parts, Body::from(bytes)))
}
