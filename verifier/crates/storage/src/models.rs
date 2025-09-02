use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct Key {
    pub key_id: Uuid,
    pub metadata: String,
}
