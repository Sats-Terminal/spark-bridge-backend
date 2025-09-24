use crate::error::GatewayError;
use crate::init::AppState;
use axum::Json;
use axum::extract::State;
use gateway_deposit_verification::error::DepositVerificationError;
use gateway_verifier_client::client::VerifierClient;
use global_utils::common_resp::Empty;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::task::JoinSet;
use tracing::{info, instrument, trace};

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct TestSparkRequest {
    pub btc_address: String,
}

#[instrument(level = "info", skip(state), ret)]
pub async fn handle(State(state): State<AppState>) -> Result<Json<Empty>, GatewayError> {
    // TODO: add check of db in 'state.deposit_verification_aggregator'
    trace!(
        "Handling healthcheck request..., {:?}",
        state.typed_verifiers_clients.keys().collect::<Vec<_>>()
    );
    check_set_of_verifiers(&state.typed_verifiers_clients)
        .await
        .map_err(|err| {
            GatewayError::HealthcheckError(format!("Verifier clients healthcheck failed: {}", err.to_string()))
        })?;
    Ok(Json(Empty {}))
}

#[instrument(level = "trace", skip(state), ret)]
pub async fn check_set_of_verifiers(state: &HashMap<u16, Arc<VerifierClient>>) -> Result<(), DepositVerificationError> {
    let mut join_set = JoinSet::new();
    // TODO: check threshold value of DepositVerificators
    for (v_id, v_client) in state.iter() {
        join_set.spawn({
            let (v_id, v_client) = (*v_id, v_client.clone());
            async move {
                v_client
                    .healthcheck()
                    .await
                    .map_err(|e| DepositVerificationError::FailedToCheckStatusOfVerifier {
                        msg: e.to_string(),
                        id: v_id,
                    })
            }
        });
    }
    let _r = join_set.join_all().await.into_iter().collect::<Result<Vec<_>, _>>()?;
    Ok(())
}
