use crate::env_parser::{EnvParser, EnvParserError};

pub struct ConfigPath {
    pub path: String,
}
impl EnvParser for ConfigPath {
    const ENV_NAME: &'static str = "CONFIG_PATH";
}
impl ConfigPath {
    /// Reads `CONFIG_PATH` env
    pub fn from_env() -> Result<Self, EnvParserError> {
        Ok(Self {
            path: ConfigPath::obtain_env_value()?,
        })
    }
}
