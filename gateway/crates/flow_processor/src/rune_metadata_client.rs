use crate::error::FlowProcessorError;
use reqwest::header::{HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use std::env;
use std::sync::Arc;
use tracing::instrument;
use url::Url;

const DEFAULT_ICON_BASE_URL: &str = "https://icon.unisat.io/icon/runes/";

#[derive(Debug, Clone, Serialize, Deserialize)]
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
        let needs_v0 = {
            let path = base_url.path();
            !(path.ends_with("/v0/") || path.ends_with("/v0"))
        };
        {
            let mut segments = base_url
                .path_segments_mut()
                .map_err(|_| FlowProcessorError::RuneMetadataError("invalid MAESTRO_API_URL".to_string()))?;
            segments.pop_if_empty();
            if needs_v0 {
                segments.push("v0");
            }
            segments.push("");
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

        let status = response.status();
        let body_bytes = response
            .bytes()
            .await
            .map_err(|err| FlowProcessorError::RuneMetadataError(err.to_string()))?;

        if !status.is_success() {
            let body = String::from_utf8_lossy(&body_bytes);
            return Err(FlowProcessorError::RuneMetadataError(format!(
                "Maestro returned {}: {}",
                status, body
            )));
        }

        let payload: Envelope<RuneResponse> = serde_json::from_slice(&body_bytes)
            .map_err(|err| FlowProcessorError::RuneMetadataError(err.to_string()))?;

        let rune = payload.data;
        let RuneResponse {
            id,
            name,
            spaced_name,
            divisibility,
            max_supply,
        } = rune;

        let divisibility = divisibility.unwrap_or(0);
        let icon_url = spaced_name
            .as_ref()
            .map(|spaced| format!("{}{}", DEFAULT_ICON_BASE_URL, urlencoding::encode(spaced)));
        let parsed_max_supply =
            parse_decimal_u128(max_supply.as_deref()).map_err(|err| FlowProcessorError::RuneMetadataError(err))?;
        let rune_name = name.unwrap_or_else(|| id.clone());

        Ok(RuneMetadata {
            id,
            name: rune_name,
            spaced_name,
            divisibility,
            max_supply: parsed_max_supply,
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

            // Remove common formatting characters that may appear in API responses.
            let sanitized = trimmed.replace([',', '_', ' '], "");

            if let Some((int_part, frac_part)) = sanitized.split_once('.') {
                if int_part.is_empty() && frac_part.is_empty() {
                    return Ok(Some(0));
                }

                if !int_part.chars().all(|c| c.is_ascii_digit()) || !frac_part.chars().all(|c| c.is_ascii_digit()) {
                    return Err(format!("Failed to parse max supply: {raw}"));
                }

                let mut combined = String::with_capacity(int_part.len() + frac_part.len());
                combined.push_str(int_part);
                combined.push_str(frac_part);

                let digits = if combined.is_empty() { "0" } else { combined.as_str() };
                digits
                    .parse::<u128>()
                    .map(Some)
                    .map_err(|err| format!("Failed to parse max supply: {err}"))
            } else {
                if !sanitized.chars().all(|c| c.is_ascii_digit()) {
                    return Err(format!("Failed to parse max supply: {raw}"));
                }
                sanitized
                    .parse::<u128>()
                    .map(Some)
                    .map_err(|err| format!("Failed to parse max supply: {err}"))
            }
        }
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::parse_decimal_u128;

    #[test]
    fn parse_decimal_u128_handles_none_and_empty() {
        assert_eq!(parse_decimal_u128(None).unwrap(), None);
        assert_eq!(parse_decimal_u128(Some("")).unwrap(), None);
        assert_eq!(parse_decimal_u128(Some("   ")).unwrap(), None);
    }

    #[test]
    fn parse_decimal_u128_handles_integer_strings() {
        assert_eq!(parse_decimal_u128(Some("42")).unwrap(), Some(42));
        assert_eq!(parse_decimal_u128(Some("0005")).unwrap(), Some(5));
    }

    #[test]
    fn parse_decimal_u128_handles_decimal_strings() {
        assert_eq!(
            parse_decimal_u128(Some("420696969696969.00")).unwrap(),
            Some(42_069_696_969_696_900)
        );
        assert_eq!(parse_decimal_u128(Some("0.01")).unwrap(), Some(1));
        assert_eq!(parse_decimal_u128(Some(".5")).unwrap(), Some(5));
    }

    #[test]
    fn parse_decimal_u128_rejects_invalid_input() {
        assert!(parse_decimal_u128(Some("abc")).is_err());
        assert!(parse_decimal_u128(Some("12.3.4")).is_err());
        assert!(parse_decimal_u128(Some("1,2,3a")).is_err());
    }
}
