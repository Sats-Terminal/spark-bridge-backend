use crate::client::VerifierClient;
use crate::error::VerifierClientError;
use tracing::instrument;

const HEALTHCHECK_PATH: &str = "/health";

impl VerifierClient {
    #[instrument(skip(self), err)]
    pub async fn healthcheck(&self) -> Result<(), VerifierClientError> {
        let url = self.get_url(HEALTHCHECK_PATH).await?;
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| VerifierClientError::HttpError(format!("Failed to send HTTP request: {:?}", e)))?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(VerifierClientError::HttpError(format!(
                "Failed to send HTTP request with status {}",
                response.status()
            )))
        }
    }
}
