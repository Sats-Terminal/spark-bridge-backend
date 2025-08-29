use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct Request {
    pub request_id: Uuid,
}