use global_utils::logger::{LoggerGuard, init_logger};
use std::sync::LazyLock;

pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");
pub static TEST_LOGGER: LazyLock<LoggerGuard> = LazyLock::new(|| init_logger());
pub const GATEWAY_CONFIG_PATH: &str = "../../../infrastructure/configurations/gateway/dev.toml";
