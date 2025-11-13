use reqwest::{Client, Method, StatusCode, Url, header};
use serde::{Serialize, de::DeserializeOwned};
use thiserror::Error;
use tracing::{debug, error};

#[derive(Debug, Error)]
pub enum HttpClientError {
    #[error("URL parse error: {0}")]
    URLParse(#[from] url::ParseError),
    #[error("Reqwest error: {0}")]
    ReqwestError(#[from] reqwest::Error),
    #[error("Reqwest invalid header error: {0}")]
    ReqwestInvalidHeaderError(#[from] reqwest::header::InvalidHeaderValue),
    #[error("Failed to parse response: {0}")]
    ParseError(String),
    #[error("Failed to do request: {url} - {status} - {body}")]
    RequestFailedError { url: Url, status: StatusCode, body: String },
}

#[derive(Debug, Clone)]
pub struct HttpClient {
    base_url: Url,
    client: Client,
}

impl HttpClient {
    pub fn new(base_url: Url) -> Self {
        Self {
            base_url,
            client: Client::new(),
        }
    }

    pub async fn get<T: DeserializeOwned>(
        &self,
        path: &str,
        query: Option<&impl Serialize>,
        headers: Option<header::HeaderMap>,
    ) -> Result<T, HttpClientError> {
        self.send_request(Method::GET, path, query, None::<&()>, headers).await
    }

    pub async fn post<T: DeserializeOwned>(
        &self,
        path: &str,
        body: Option<&impl Serialize>,
        headers: Option<header::HeaderMap>,
    ) -> Result<T, HttpClientError> {
        self.send_request(Method::POST, path, None::<&()>, body, headers).await
    }

    async fn send_request<T: DeserializeOwned>(
        &self,
        method: reqwest::Method,
        path: &str,
        query: Option<&impl Serialize>,
        body: Option<&impl Serialize>,
        headers: Option<header::HeaderMap>,
    ) -> Result<T, HttpClientError> {
        let url = self.base_url.join(path)?;
        debug!(?method, ?url, "performing request");

        let mut request = self.client.request(method, url.clone());

        if let Some(h) = headers {
            request = request.headers(h);
        }
        if let Some(q) = query {
            request = request.query(q);
        }
        if let Some(b) = body {
            request = request.json(b);
        }

        let response = request.send().await?;

        let status = response.status();
        match response.status() {
            StatusCode::OK => match response.json::<T>().await {
                Ok(parsed) => {
                    debug!(?url, ?status, "Request successful");
                    Ok(parsed)
                }
                Err(err) => {
                    error!(?err, "Failed to parse successful response");
                    Err(err.into())
                }
            },
            _ => {
                let body_text = response.text().await.unwrap_or_else(|_| "N/A".to_string());
                error!(?url, ?status, body = body_text, "Request failed");

                Err(HttpClientError::RequestFailedError {
                    url,
                    status,
                    body: body_text,
                })
            }
        }
    }
}
