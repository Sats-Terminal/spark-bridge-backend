use crate::storage::LocalDbStorage;
use async_trait::async_trait;
use bitcoin::secp256k1::PublicKey;
use bitcoin::secp256k1::XOnlyPublicKey;
use persistent_storage::error::DbError;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::str::FromStr;
use tracing::instrument;
use uuid::Uuid;

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum UserId {
    Uuid(Uuid),
    PublicKey(PublicKey),
    XOnlyPublicKey(XOnlyPublicKey),
}

impl ToString for UserId {
    fn to_string(&self) -> String {
        match self {
            UserId::Uuid(uuid) => uuid.to_string(),
            UserId::PublicKey(public_key) => public_key.to_string(),
            UserId::XOnlyPublicKey(x_only_public_key) => x_only_public_key.to_string(),
        }
    }
}

impl FromStr for UserId {
    type Err = DbError;
    fn from_str(s: &str) -> Result<Self, DbError> {
        let uuid_response = Uuid::from_str(s);
        if uuid_response.is_ok() {
            return Ok(UserId::Uuid(uuid_response.unwrap()));
        }
        let public_key_response = PublicKey::from_str(s);
        if public_key_response.is_ok() {
            return Ok(UserId::PublicKey(public_key_response.unwrap()));
        }
        let x_only_public_key_response = XOnlyPublicKey::from_str(s);
        if x_only_public_key_response.is_ok() {
            return Ok(UserId::XOnlyPublicKey(x_only_public_key_response.unwrap()));
        }
        Err(DbError::DecodeError(format!(
            "Invalid user id, it is not uuid, public key or x only public key: {}",
            s
        )))
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct UserIds {
    pub user_id: UserId,
    pub dkg_share_id: Uuid,
    pub rune_id: String,
    pub is_issuer: bool,
}

#[async_trait]
pub trait UserIdentifierStorage: Send + Sync + Debug {
    async fn get_row_by_user_id(&self, user_id: UserId, rune_id: &str) -> Result<Option<UserIds>, DbError>;
    async fn get_row_by_dkg_id(&self, dkg_share_id: Uuid) -> Result<Option<UserIds>, DbError>;
    async fn insert_row(&self, user_ids: &UserIds) -> Result<(), DbError>;
    async fn get_issuer_ids(&self, rune_id: &str) -> Result<Option<UserIds>, DbError>;
}

#[async_trait]
impl UserIdentifierStorage for LocalDbStorage {
    #[instrument(level = "trace", skip(self), ret)]
    async fn insert_row(&self, user_ids: &UserIds) -> Result<(), DbError> {
        let _ = sqlx::query(
            "INSERT INTO gateway.user_identifier (user_id, dkg_share_id, rune_id, is_issuer)
            VALUES ($1, $2, $3, $4)",
        )
        .bind(user_ids.user_id.to_string())
        .bind(user_ids.dkg_share_id)
        .bind(&user_ids.rune_id)
        .bind(user_ids.is_issuer)
        .execute(&self.get_conn().await?)
        .await
        .map_err(|e| DbError::BadRequest(e.to_string()))?;

        Ok(())
    }

    #[instrument(level = "trace", skip(self), ret)]
    async fn get_row_by_dkg_id(&self, dkg_share_id: Uuid) -> Result<Option<UserIds>, DbError> {
        let result: Option<(String, Uuid, String, bool)> = sqlx::query_as(
            "SELECT user_id, dkg_share_id, rune_id, is_issuer FROM gateway.user_identifier WHERE dkg_share_id = $1;",
        )
        .bind(dkg_share_id)
        .fetch_optional(&self.get_conn().await?)
        .await
        .map_err(|e| DbError::BadRequest(e.to_string()))?;

        match result {
            Some((user_id, dkg_share_id, rune_id, is_issuer)) => Ok(Some(UserIds {
                user_id: UserId::from_str(&user_id)?,
                dkg_share_id,
                rune_id,
                is_issuer,
            })),
            None => Ok(None),
        }
    }

    async fn get_row_by_user_id(&self, user_id: UserId, rune_id: &str) -> Result<Option<UserIds>, DbError> {
        let result: Option<(String, Uuid, String, bool,)> = sqlx::query_as(
            "SELECT user_id, dkg_share_id, rune_id, is_issuer FROM gateway.user_identifier WHERE user_id = $1 AND rune_id = $2;",
        )
            .bind(user_id.to_string())
            .bind(rune_id)
            .fetch_optional(&self.get_conn().await?)
            .await
            .map_err(|e| DbError::BadRequest(e.to_string()))?;

        match result {
            Some((user_id, dkg_share_id, rune_id, is_issuer)) => Ok(Some(UserIds {
                user_id: UserId::from_str(&user_id)?,
                dkg_share_id,
                rune_id,
                is_issuer,
            })),
            None => Ok(None),
        }
    }

    #[instrument(level = "trace", skip(self))]
    async fn get_issuer_ids(&self, rune_id: &str) -> Result<Option<UserIds>, DbError> {
        let result: Option<(String, Uuid, String, bool)> = sqlx::query_as(
            "SELECT user_id, dkg_share_id, rune_id, is_issuer
            FROM gateway.user_identifier
            WHERE is_issuer = true AND rune_id = $1",
        )
        .bind(rune_id)
        .fetch_optional(&self.get_conn().await?)
        .await
        .map_err(|e| DbError::BadRequest(e.to_string()))?;

        match result {
            Some((user_id, dkg_share_id, rune_id, is_issuer)) => Ok(Some(UserIds {
                user_id: UserId::from_str(&user_id)?,
                dkg_share_id,
                rune_id,
                is_issuer,
            })),
            None => Ok(None),
        }
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
    async fn test_aggregator_signer_integration(db: PostgresPool) -> eyre::Result<()> {
        let tweak = None;
        _test_aggregator_signer_integration(db, tweak).await
    }

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn test_aggregator_signer_integration_tweaked(db: PostgresPool) -> eyre::Result<()> {
        let tweak = Some(generate_tweak_bytes());
        _test_aggregator_signer_integration(db, tweak).await
    }

    async fn _test_aggregator_signer_integration(db: sqlx::PgPool, tweak: Option<TweakBytes>) -> eyre::Result<()> {
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
