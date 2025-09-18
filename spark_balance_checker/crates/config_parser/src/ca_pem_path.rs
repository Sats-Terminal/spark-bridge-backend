use global_utils::env_parser::{EnvParser, EnvParserError};

pub struct CaPemPath {
    pub path: String,
}
impl EnvParser for CaPemPath {
    const ENV_NAME: &'static str = "CA_PEM_PATH";
}
impl CaPemPath {
    /// Reads CA_PEM_PATH env
    pub fn from_env() -> Result<Self, EnvParserError> {
        Ok(Self {
            path: CaPemPath::obtain_env_value()?,
        })
    }
}
