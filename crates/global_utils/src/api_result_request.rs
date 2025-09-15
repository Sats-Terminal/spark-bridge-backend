use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

pub trait ErrorIntoStatusMsgTuple {
    fn into_status_msg_tuple(self) -> (axum::http::StatusCode, String);
}

#[derive(Debug, Clone, Serialize, Eq, PartialEq, ToSchema)]
pub enum ApiResponse<'a, T> {
    #[serde(rename = "ok")]
    Ok { data: &'a T },
    #[serde(rename = "err")]
    Err { code: u16, message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub enum ApiResponseOwned<T> {
    #[serde(rename = "ok")]
    Ok { data: T },
    #[serde(rename = "err")]
    Err { code: u16, message: String },
}

#[derive(Serialize, ToSchema, Debug)]
#[schema(example = json!({ }))]
pub struct Empty {}

impl<'a, T: Serialize> ApiResponse<'a, T> {
    pub fn ok(data: &'a T) -> ApiResponse<'a, T> {
        Self::Ok { data }
    }

    pub fn not_found() -> Self {
        Self::err(404, "Not found".to_string())
    }

    pub fn unauthorized(msg: impl AsRef<str>) -> Self {
        Self::err(401, format!("Unauthorized: [{}]", msg.as_ref()))
    }

    pub fn err<S: ToString>(code: u16, message: S) -> Self {
        Self::Err {
            code,
            message: message.to_string(),
        }
    }

    pub fn encode_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("Failed to serialize value")
    }

    pub fn encode_string_json(&self) -> String {
        serde_json::to_string(self).expect("Failed to serialize value")
    }
}

impl<'a, T: Deserialize<'a> + Serialize + Clone> ApiResponseOwned<T> {
    pub fn ok(data: T) -> ApiResponseOwned<T> {
        Self::Ok { data }
    }

    pub fn not_found() -> Self {
        Self::err(404, "Not found".to_string())
    }

    pub fn unauthorized() -> Self {
        Self::err(401, "Unauthorized".to_string())
    }

    pub fn err<S: ToString>(code: u16, message: S) -> Self {
        Self::Err {
            code,
            message: message.to_string(),
        }
    }

    pub fn encode_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("Failed to serialize value")
    }

    pub fn encode_string_json(&self) -> String {
        serde_json::to_string(self).expect("Failed to serialize value")
    }
}

impl<'a, T: Deserialize<'a> + Serialize + Clone, E: ErrorIntoStatusMsgTuple> From<Result<T, E>>
    for ApiResponseOwned<T>
{
    fn from(value: Result<T, E>) -> Self {
        match value {
            Ok(v) => ApiResponseOwned::ok(v),
            Err(e) => {
                let (code, msg) = e.into_status_msg_tuple();
                ApiResponseOwned::<T>::err(code.as_u16(), msg)
            }
        }
    }
}

/// Uses result from indexer and builds Json encoded string as response
fn _result_into_json<T: Serialize, E: ErrorIntoStatusMsgTuple>(res: Result<T, E>) -> String {
    match res {
        Ok(v) => ApiResponse::ok(&v).encode_string_json(),
        Err(e) => {
            let (code, msg) = e.into_status_msg_tuple();
            ApiResponse::<String>::err(code.as_u16(), msg).encode_string_json()
        }
    }
}
