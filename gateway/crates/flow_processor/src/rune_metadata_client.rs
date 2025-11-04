use crate::error::FlowProcessorError;
use reqwest::header::{HeaderMap, HeaderValue};
use serde::Deserialize;
use std::env;
use std::sync::Arc;
use tracing::instrument;
use url::Url;

const DEFAULT_ICON_BASE_URL: &str = "https://icon.unisat.io/icon/runes/";

#[derive(Debug, Clone)]
pub struct RuneMetadata {
    pub id: String,
    pub name: String,
    pub spaced_name: Option<String>,
    pub divisibility: u8,
    pub max_supply: Option<u128>,
    pub icon_url: Option<String>,
}

#[derive(Clone, Debug)]
pub struct RuneMetadataClient {
    base_url: Url,
    http: Arc<reqwest::Client>,
}

#[derive(Deserialize)]
struct Envelope<T> {
    data: T,
}

#[derive(Deserialize)]
struct RuneResponse {
    id: String,
    name: Option<String>,
    spaced_name: Option<String>,
    divisibility: Option<u8>,
    max_supply: Option<String>,
}

impl RuneMetadataClient {
    pub fn from_env() -> Result<Option<Self>, FlowProcessorError> {
        let base_url = match env::var("MAESTRO_API_URL") {
            Ok(url) => url,
            Err(_) => return Ok(None),
        };
        let api_key = env::var("MAESTRO_API_KEY").map_err(|_| {
            FlowProcessorError::RuneMetadataError("MAESTRO_API_KEY environment variable is not set".to_string())
        })?;

        let mut default_headers = HeaderMap::new();
        default_headers.insert(
            "api-key",
            HeaderValue::from_str(&api_key).map_err(|err| FlowProcessorError::RuneMetadataError(err.to_string()))?,
        );

        let client = reqwest::Client::builder()
            .default_headers(default_headers)
            .build()
            .map_err(|err| FlowProcessorError::RuneMetadataError(err.to_string()))?;
        let mut base_url =
            Url::parse(&base_url).map_err(|err| FlowProcessorError::RuneMetadataError(err.to_string()))?;
        if !base_url.path().ends_with('/') {
            base_url
                .path_segments_mut()
                .map_err(|_| FlowProcessorError::RuneMetadataError("invalid MAESTRO_API_URL".to_string()))?
                .push("");
        }

        Ok(Some(Self {
            base_url,
            http: Arc::new(client),
        }))
    }

    #[instrument(level = "trace", skip(self))]
    pub async fn get_metadata(&self, rune_id: &str) -> Result<RuneMetadata, FlowProcessorError> {
        let url = self
            .base_url
            .join(&format!("assets/runes/{rune_id}"))
            .map_err(|err| FlowProcessorError::RuneMetadataError(err.to_string()))?;

        let response = self
            .http
            .get(url)
            .send()
            .await
            .map_err(|err| FlowProcessorError::RuneMetadataError(err.to_string()))?;
        if !response.status().is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(FlowProcessorError::RuneMetadataError(format!(
                "Maestro returned {}: {}",
                response.status(),
                body
            )));
        }

        let payload: Envelope<RuneResponse> = response
            .json()
            .await
            .map_err(|err| FlowProcessorError::RuneMetadataError(err.to_string()))?;

        let divisibility = payload.divisibility.unwrap_or(0);
        let name = payload.name.clone().unwrap_or_else(|| payload.id.clone());
        let icon_url = payload
            .spaced_name
            .clone()
            .map(|spaced| format!("{}{}", DEFAULT_ICON_BASE_URL, urlencoding::encode(&spaced)));

        Ok(RuneMetadata {
            id: payload.id,
            name,
            spaced_name: payload.spaced_name,
            divisibility,
            max_supply: parse_decimal_u128(payload.max_supply.as_deref())
                .map_err(|err| FlowProcessorError::RuneMetadataError(err))?,
            icon_url,
        })
    }
}

fn parse_decimal_u128(value: Option<&str>) -> Result<Option<u128>, String> {
    match value {
        Some(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                return Ok(None);
            }
            trimmed
                .parse::<u128>()
                .map(Some)
                .map_err(|err| format!("Failed to parse max supply: {err}"))
        }
        None => Ok(None),
    }
}
