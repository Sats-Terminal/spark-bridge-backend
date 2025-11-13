use crate::storage::LocalDbStorage;
use async_trait::async_trait;
use frost::traits::SignerDkgShareStorage;
use frost::types::SignerDkgShareIdData;
use frost::types::SignerDkgState;
use persistent_storage::error::DbError;
use sqlx::types::Json;
use uuid::Uuid;

#[async_trait]
impl SignerDkgShareStorage for LocalDbStorage {
    async fn get_dkg_share_signer_data(&self, dkg_share_id: &Uuid) -> Result<Option<SignerDkgShareIdData>, DbError> {
        let result: Option<(Json<SignerDkgState>,)> = sqlx::query_as(
            "SELECT dkg_signer_state
            FROM  verifier.dkg_share
            WHERE dkg_share_id = $1",
        )
        .bind(dkg_share_id)
        .fetch_optional(&self.get_conn().await?)
        .await
        .map_err(|e| DbError::BadRequest(e.to_string()))?;

        Ok(result.map(|(json_dkg_state,)| SignerDkgShareIdData {
            dkg_state: json_dkg_state.0,
        }))
    }

    async fn set_dkg_share_signer_data(
        &self,
        dkg_share_id: &Uuid,
        dkg_share_data: SignerDkgShareIdData,
    ) -> Result<(), DbError> {
        let _ = sqlx::query(
            "INSERT INTO verifier.dkg_share (dkg_share_id, dkg_signer_state)
            VALUES ($1, $2)
            ON CONFLICT (dkg_share_id) DO UPDATE SET dkg_signer_state = $2",
        )
        .bind(dkg_share_id)
        .bind(Json(dkg_share_data.dkg_state))
        .execute(&self.get_conn().await?)
        .await
        .map_err(|e| DbError::BadRequest(e.to_string()))?;
        Ok(())
    }
}
