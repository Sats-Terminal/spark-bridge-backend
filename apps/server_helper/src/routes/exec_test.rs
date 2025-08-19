use actix_web::{HttpResponseBuilder, Responder, http::StatusCode, web};
use script_executor::test_executor::CmdOutput;
use serde::{Deserialize, Serialize};
use tracing::{debug, instrument};
use utoipa::ToSchema;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ExecuteTestRequest {
    artillery_test_path: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub enum ExecuteTestResponse {
    Ok(CmdOutput),
    Err(String),
}

/// Handler for the `/exec_test` endpoint returns a result type to indicate status of test update execution.
#[utoipa::path(
    get,
    operation_id = "exec_test",
    path = "/exec_test",
    tag = "artillery_test_execution",
    request_body(
        content = ExecuteTestRequest,
        description = "Path of artillery test for execution in spark",
        content_type = "application/json",
        example = json!({ "artillery_test_path": "some_path" })
    ),
    responses(
        (
            status = 200,
            description = "Backend is successfully executed transaction",
            body = ExecuteTestResponse,
            content_type = "application/json",
        ),
    ),
)]
#[instrument(level = "debug", ret)]
pub async fn handle(body: web::Json<ExecuteTestRequest>) -> impl Responder {
    debug!("Processing 'POST /exec_test with body {body:?}'");
    let test_path = body.into_inner().artillery_test_path;
    match inner_execute(&test_path) {
        Ok(cmd_out) => HttpResponseBuilder::new(StatusCode::OK).json(ExecuteTestResponse::Ok(cmd_out)),
        Err(e) => HttpResponseBuilder::new(StatusCode::INTERNAL_SERVER_ERROR).json(ExecuteTestResponse::Err(format!(
            "Occurred error in (POST /exec_test with body {test_path:?}): {e}"
        ))),
    }
}

fn inner_execute(_path_to_execute: &str) -> crate::error::Result<CmdOutput> {
    Ok(CmdOutput {
        status: None,
        stdout: "All ok".to_string(),
        stderr: "".to_string(),
    })
}
