use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct Request {
    pub request_id: Uuid,
    pub key_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct Key {
    pub key_id: Uuid,
}