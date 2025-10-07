use frost::mocks::{MockSignerClient, MockSignerDkgShareIdStorage, MockSignerSignSessionStorage};
use frost::signer::FrostSigner;
use frost::traits::SignerClient;
use frost_secp256k1_tr::Identifier;
use global_utils::logger::{LoggerGuard, init_logger};
use std::collections::BTreeMap;
use std::sync::{Arc, LazyLock};

pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");
pub static TEST_LOGGER: LazyLock<LoggerGuard> = LazyLock::new(|| init_logger());
pub const GATEWAY_CONFIG_PATH: &str = "../../../infrastructure/configurations/gateway/dev.toml";

pub async fn create_mock_signer(identifier: u16) -> FrostSigner {
    FrostSigner::new(
        identifier,
        Arc::new(MockSignerDkgShareIdStorage::default()),
        Arc::new(MockSignerSignSessionStorage::default()),
        3,
        2,
    )
    .unwrap()
}

pub async fn create_mock_verifiers_map() -> BTreeMap<Identifier, Arc<dyn SignerClient>> {
    let signer1 = create_mock_signer(1).await;
    let signer2 = create_mock_signer(2).await;
    let signer3 = create_mock_signer(3).await;

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
