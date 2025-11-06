use global_utils::logger::{LoggerGuard, init_logger};
use sqlx::migrate::Migrator;
use std::sync::LazyLock;

pub static TEST_LOGGER: LazyLock<LoggerGuard> = LazyLock::new(|| init_logger());
pub static MIGRATOR: Migrator = sqlx::migrate!("../local_db_store/migrations");

pub const CONFIG_PATH: &str = "../../../infrastructure/configurations/gateway/dev.toml";
