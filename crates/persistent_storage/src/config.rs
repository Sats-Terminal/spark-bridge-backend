use global_utils::env_parser::{EnvParser, EnvParserError};
use serde::{Deserialize, Serialize};

pub const POSTGRES_TESTING_URL_ENV_NAME: &str = "DATABASE_URL_TESTING";
pub const POSTGRES_URL_ENV_NAME: &str = "DATABASE_URL";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PostgresDbTestingCredentials {
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PostgresDbCredentials {
    pub url: String,
}

impl EnvParser for PostgresDbTestingCredentials {
    const ENV_NAME: &'static str = POSTGRES_TESTING_URL_ENV_NAME;
}

impl EnvParser for PostgresDbCredentials {
    const ENV_NAME: &'static str = POSTGRES_URL_ENV_NAME;
}

impl PostgresDbTestingCredentials {
    pub fn new() -> Result<Self, EnvParserError> {
        Ok(Self {
            url: Self::obtain_env_value()?,
        })
    }
}

impl PostgresDbCredentials {
    pub fn new() -> Result<Self, EnvParserError> {
        Ok(Self {
            url: Self::obtain_env_value()?,
        })
    }
}
