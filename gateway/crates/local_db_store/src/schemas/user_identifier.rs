use crate::schemas::musig_id::MusigId;
use crate::storage::LocalDbStorage;
use async_trait::async_trait;
use frost::types::DkgShareId;
use persistent_storage::error::DbError;
use serde::{Deserialize, Serialize};
use sqlx::types::Uuid;
use std::fmt::Debug;
use tracing::instrument;

pub type UserUuid = Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserUniqueId {
    pub uuid: UserUuid,
    pub rune_id: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct UserIdentifier {
    pub user_uuid: Uuid,
    pub dkg_share_id: Uuid,
    //todo: remove
    pub public_key: String,
    pub rune_id: String,
    pub is_issuer: bool,
}

#[derive(Debug, Clone)]
pub struct UserIdentifierData {
    pub public_key: String,
    pub rune_id: String,
    pub is_issuer: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserIds {
    pub user_uuid: Uuid,
    pub dkg_share_id: Uuid,
    pub rune_id: String,
}

#[async_trait]
pub trait UserIdentifierStorage: Send + Sync + Debug {
    async fn get_row_by_user_unique_id(&self, dkg_share_id: &UserUniqueId) -> Result<Option<UserIdentifier>, DbError>;
    async fn get_dkg_share_data_via_dkg_share(
        &self,
        dkg_share_id: &DkgShareId,
    ) -> Result<Option<UserIdentifier>, DbError>;
    async fn set_user_identifier_data(
        &self,
        user_identifier: &UserUuid,
        dkg_share_id: &DkgShareId,
        user_identifier_data: UserIdentifierData,
    ) -> Result<(), DbError>;
    async fn get_issuer_ids(&self, rune_id: String) -> Result<Option<UserIds>, DbError>;
    async fn get_ids_by_musig_id(&self, musig_id: &MusigId) -> Result<Option<UserIds>, DbError>;
}

#[async_trait]
impl UserIdentifierStorage for LocalDbStorage {
    #[instrument(level = "trace", skip(self), ret)]
    async fn set_user_identifier_data(
        &self,
        user_identifier: &UserUuid,
        dkg_share_id: &DkgShareId,
        user_identifier_data: UserIdentifierData,
    ) -> Result<(), DbError> {
        let _ = sqlx::query(
            "INSERT INTO gateway.user_identifier (user_uuid, dkg_share_id, public_key, rune_id, is_issuer)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (user_uuid, rune_id) DO NOTHING",
        )
        .bind(user_identifier)
        .bind(dkg_share_id)
        .bind(user_identifier_data.public_key)
        .bind(user_identifier_data.rune_id)
        .bind(user_identifier_data.is_issuer)
        .execute(&self.get_conn().await?)
        .await
        .map_err(|e| DbError::BadRequest(e.to_string()))?;

        Ok(())
    }

    #[instrument(level = "trace", skip(self), ret)]
    async fn get_row_by_user_unique_id(&self, id: &UserUniqueId) -> Result<Option<UserIdentifier>, DbError> {
        let result: Option<(Uuid, Uuid, String, String, bool,)> = sqlx::query_as(
            "SELECT user_uuid, dkg_share_id, public_key, rune_id, is_issuer FROM gateway.user_identifier WHERE user_uuid = $1 AND rune_id = $2;",
        )
            .bind(id.uuid)
            .bind(&id.rune_id)
            .fetch_optional(&self.get_conn().await?)
            .await
            .map_err(|e| DbError::BadRequest(e.to_string()))?;
        Ok(result.map(
            |(user_uuid, dkg_share_id, public_key, rune_id, is_issuer)| UserIdentifier {
                user_uuid,
                dkg_share_id,
                public_key,
                rune_id,
                is_issuer,
            },
        ))
    }

    async fn get_dkg_share_data_via_dkg_share(
        &self,
        dkg_share_id: &DkgShareId,
    ) -> Result<Option<UserIdentifier>, DbError> {
        let result: Option<(Uuid, Uuid, String, String, bool,)> = sqlx::query_as(
            "SELECT user_uuid, dkg_share_id, public_key, rune_id, is_issuer FROM gateway.user_identifier WHERE dkg_share_id = $1;",
        )
            .bind(dkg_share_id)
            .fetch_optional(&self.get_conn().await?)
            .await
            .map_err(|e| DbError::BadRequest(e.to_string()))?;

        Ok(result.map(
            |(user_uuid, dkg_share_id, public_key, rune_id, is_issuer)| UserIdentifier {
                user_uuid,
                dkg_share_id,
                public_key,
                rune_id,
                is_issuer,
            },
        ))
    }

    #[instrument(level = "trace", skip(self))]
    async fn get_issuer_ids(&self, rune_id: String) -> Result<Option<UserIds>, DbError> {
        let result: Option<(UserUuid, DkgShareId, String)> = sqlx::query_as(
            "SELECT user_uuid, dkg_share_id, rune_id
            FROM gateway.user_identifier
            WHERE is_issuer = true AND rune_id = $1",
        )
        .bind(rune_id)
        .fetch_optional(&self.get_conn().await?)
        .await
        .map_err(|e| DbError::BadRequest(e.to_string()))?;

        Ok(result.map(|(user_uuid, dkg_share_id, rune_id)| UserIds {
            user_uuid,
            dkg_share_id,
            rune_id,
        }))
    }

    #[instrument(level = "trace", skip(self))]
    async fn get_ids_by_musig_id(&self, musig_id: &MusigId) -> Result<Option<UserIds>, DbError> {
        let rune_id = musig_id.get_rune_id();
        let pubkey = musig_id.get_public_key();
        let is_issuer = musig_id.is_issuer();
        let result: Option<(UserUuid, DkgShareId, String)> = sqlx::query_as(
            "SELECT user_uuid, dkg_share_id, rune_id
            FROM gateway.user_identifier
            WHERE is_issuer = $1 AND rune_id = $2 AND public_key = $3",
        )
        .bind(is_issuer)
        .bind(rune_id)
        .bind(pubkey.to_string())
        .fetch_optional(&self.get_conn().await?)
        .await
        .map_err(|e| DbError::BadRequest(e.to_string()))?;

        Ok(result.map(|(user_uuid, dkg_share_id, rune_id)| UserIds {
            user_uuid,
            dkg_share_id,
            rune_id,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schemas::dkg_share::DkgShareGenerate;
    use bitcoin::secp256k1::{Secp256k1, SecretKey};
    use frost::aggregator::FrostAggregator;
    use frost::mocks::*;
    use frost::signer::FrostSigner;
    use frost::traits::SignerClient;
    use frost::traits::*;
    use frost::types::{SigningMetadata, TweakBytes};
    use frost::utils::generate_tweak_bytes;
    use frost_secp256k1_tr::Identifier;
    use frost_secp256k1_tr::keys::Tweak;
    use gateway_config_parser::config::ServerConfig;
    use persistent_storage::init::{PostgresPool, PostgresRepo};
    use std::collections::BTreeMap;
    use std::sync::Arc;

    pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");
    pub const GATEWAY_CONFIG_PATH: &str = "../../../infrastructure/configurations/gateway/dev.toml";

    async fn create_signer(identifier: u16) -> FrostSigner {
        FrostSigner::new(
            identifier,
            Arc::new(MockSignerDkgShareIdStorage::default()),
            Arc::new(MockSignerSignSessionStorage::default()),
            3,
            2,
        )
        .unwrap()
    }

    async fn create_verifiers_map_easy() -> BTreeMap<Identifier, Arc<dyn SignerClient>> {
        let signer1 = create_signer(1).await;
        let signer2 = create_signer(2).await;
        let signer3 = create_signer(3).await;

        let mock_signer_client1 = MockSignerClient::new(signer1);
        let mock_signer_client2 = MockSignerClient::new(signer2);
        let mock_signer_client3 = MockSignerClient::new(signer3);

        let identifier_1: Identifier = 1.try_into().unwrap();
        let identifier_2: Identifier = 2.try_into().unwrap();
        let identifier_3: Identifier = 3.try_into().unwrap();

        BTreeMap::from([
            (identifier_1, Arc::new(mock_signer_client1) as Arc<dyn SignerClient>),
            (identifier_2, Arc::new(mock_signer_client2) as Arc<dyn SignerClient>),
            (identifier_3, Arc::new(mock_signer_client3) as Arc<dyn SignerClient>),
        ])
    }

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn test_aggregator_signer_integration(db: PostgresPool) -> anyhow::Result<()> {
        let tweak = None;
        _test_aggregator_signer_integration(db, tweak).await
    }

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn test_aggregator_signer_integration_tweaked(db: PostgresPool) -> anyhow::Result<()> {
        let tweak = Some(generate_tweak_bytes());
        _test_aggregator_signer_integration(db, tweak).await
    }

    async fn _test_aggregator_signer_integration(db: sqlx::PgPool, tweak: Option<TweakBytes>) -> anyhow::Result<()> {
        let server_config = ServerConfig::init_config(GATEWAY_CONFIG_PATH.to_string());
        let local_repo = Arc::new(LocalDbStorage {
            postgres_repo: PostgresRepo { pool: db },
            network: server_config.network.network,
        });

        let user_id = local_repo.generate_dkg_share_entity().await?;

        let verifiers_map = create_verifiers_map_easy().await;
        let aggregator = FrostAggregator::new(verifiers_map, local_repo.clone(), local_repo);

        let secp = Secp256k1::new();
        let secret_key = SecretKey::from_slice(&[4u8; 32])?;

        //let user_id = "test_user";
        let message_hash = b"test_message";

        let public_key_package = aggregator.run_dkg_flow(&user_id).await?;

        let metadata = SigningMetadata::Authorization;

        let signature = aggregator
            .run_signing_flow(user_id, message_hash, metadata, tweak, false)
            .await?;

        let tweaked_public_key_package = match tweak.clone() {
            Some(tweak) => public_key_package.clone().tweak(Some(tweak.to_vec())),
            None => public_key_package.clone(),
        };
        tweaked_public_key_package
            .verifying_key()
            .verify(message_hash, &signature)?;

        Ok(())
    }
}
