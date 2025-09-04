use uuid::Uuid;
use serde::{Deserialize, Serialize};
use frost::traits::SignerUserStorage;
use crate::Storage;
use crate::DatabaseError;
use frost::traits::SignerUserState;
use persistent_storage::init::PostgresRepo;

#[async_trait::async_trait]
impl SignerUserStorage for Storage {
    async fn get_user_state(&self, user_public_key: String) -> Result<Option<SignerUserState>, DatabaseError> {
        let result: Option<(String,)> = sqlx::query_as("SELECT state_data FROM verifier.keys WHERE user_public_key = $1")
            .bind(user_public_key)
            .fetch_optional(&self.get_conn().await?)
            .await
            .map_err(|e| DatabaseError::BadRequest(e.to_string()))?;

        let state: Option<SignerUserState> = if let Some((state_data, )) = result {
            Some(serde_json::from_str(&state_data)
                .map_err(|e| DatabaseError::BadRequest(format!("Failed to deserialize state: {}", e)))?)
        } else {
            None
        };

        Ok(state)
    }

    async fn set_user_state(&self, user_public_key: String, user_state: SignerUserState) -> Result<(), DatabaseError> {
        let state_data = serde_json::to_string(&user_state)
            .map_err(|e| DatabaseError::BadRequest(format!("Failed to serialize state: {}", e)))?;
        
        let _ = sqlx::query("INSERT INTO verifier.keys (user_public_key, state_data) VALUES ($1, $2) ON CONFLICT (user_public_key) DO UPDATE SET state_data = $2")
            .bind(user_public_key)
            .bind(state_data)
            .execute(&self.get_conn().await?)
            .await
            .map_err(|e| DatabaseError::BadRequest(e.to_string()))?;

        Ok(())
    }
}
