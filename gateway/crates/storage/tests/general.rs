use tokio;
use gateway_storage::Storage;
use sqlx::PgPool;
use uuid::Uuid;
use gateway_storage::models::Key;
use gateway_storage::traits::KeyStorage;

#[tokio::test]
async fn test() {
    let url = "postgresql://postgres:postgres@localhost:5433/postgres";
    let pool = PgPool::connect(url).await.unwrap();
    let storage = Storage::new(pool);

    let key_id = Uuid::new_v4();
    let key = Key {
        key_id,
    };

    storage.insert_key(key).await.unwrap();

    let key = storage.get_key(key_id).await.unwrap();
    assert_eq!(key.key_id, key_id);
}