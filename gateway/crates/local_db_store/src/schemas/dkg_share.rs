pub struct DkgShare {}

pub trait DkgShareGenerate {
    // TODO
}

mod tests {
    use global_utils::common_types::get_uuid;
    use global_utils::logger::{LoggerGuard, init_logger};
    use persistent_storage::error::DbError as DatabaseError;
    use persistent_storage::init::{PostgresPool, PostgresRepo};
    use serde_json::json;
    use std::sync::{Arc, LazyLock};

    pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");
    pub static TEST_LOGGER: LazyLock<LoggerGuard> = LazyLock::new(|| init_logger());

    #[sqlx::test(migrator = "MIGRATOR")]
    async fn test_create_session(db: PostgresPool) -> Result<(), DatabaseError> {
        let _logger_guard = &*TEST_LOGGER;
        let repo = Arc::new(PostgresRepo { pool: db });

        Ok(())
    }
}
