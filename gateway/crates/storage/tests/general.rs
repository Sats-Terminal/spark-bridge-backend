use gateway_storage::{models::Key, traits::KeyStorage};
use persistent_storage::init::PostgresRepo;
use persistent_storage::config::PostgresDbCredentials;
use tokio;
use uuid::Uuid;

#[tokio::test]
async fn test() {
    let url = "postgresql://postgres:postgres@localhost:5433/postgres".to_string();
    let storage = PostgresRepo::from_config(PostgresDbCredentials {
        url
    }).await.unwrap();

    let key_id = Uuid::new_v4();
    let key = Key { key_id };

    storage.insert_key(key).await.unwrap();

    let key = storage.get_key(key_id).await.unwrap();
    assert_eq!(key.key_id, key_id);
}
