pub mod client;
mod common;
mod connection;
mod utils;

pub use common::{
    config::{CaCertificate, SparkConfig, SparkOperatorConfig},
    error::SparkClientError,
};
