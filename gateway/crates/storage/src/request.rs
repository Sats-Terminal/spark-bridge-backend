use crate::models::Request;
use crate::traits::RequestStorage;
use crate::errors::DatabaseError;
use uuid::Uuid;
use crate::Storage;

impl RequestStorage for Storage {
    async fn insert_request(&self, request: Request) -> Result<(), DatabaseError> {
        let pool = self.pool.lock().unwrap();
        let _ = sqlx::query!(
            "INSERT INTO requests (request_id) VALUES ($1)",
            request.request_id
        ).execute(&*pool).await.map_err(|e| DatabaseError::BadRequest(e.to_string()))?;

        Ok(())
    }

    async fn get_request(&self, request_id: Uuid) -> Result<Request, DatabaseError> {
        let pool = self.pool.lock().unwrap();
        let result = sqlx::query!(
            "SELECT * FROM requests WHERE request_id = $1 LIMIT 1",
            request_id
        ).fetch_one(&*pool).await.map_err(|e| {
            match e {
                sqlx::Error::RowNotFound => DatabaseError::NotFound(e.to_string()),
                _ => DatabaseError::BadRequest(e.to_string()),
            }
        })?;

        Ok(Request {
            request_id: result.request_id,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::PgPool;

    #[tokio::test]
    async fn test() {
        let url = "postgresql://postgres:postgres@localhost:5433/postgres";
        let pool = PgPool::connect(url).await.unwrap();
        let storage = Storage::new(pool);

        let request_id = Uuid::new_v4();
        let request_id_not_found = Uuid::new_v4();

        let request = Request {
            request_id,
        };

        storage.insert_request(request).await.unwrap();

        let request = storage.get_request(request_id).await.unwrap();
        assert_eq!(request.request_id, request_id);

        let request = storage.get_request(request_id_not_found).await.unwrap_err();
        
        if let DatabaseError::NotFound(e) = request {
            println!("{}", e);
        } else {
            panic!("Expected DatabaseError::NotFound, got {:?}", request);
        }
    }
}