use serde::Serialize;
use serde_json::json;
use utoipa::ToSchema;

#[derive(Serialize, ToSchema, Debug)]
#[schema(example = json!({ }))]
pub struct Empty {}

pub trait ErrorIntoStatusMsgTuple {
    fn into_status_msg_tuple(self) -> (axum::http::StatusCode, String);
}
