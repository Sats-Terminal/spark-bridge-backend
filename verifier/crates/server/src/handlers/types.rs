use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct WatchSparkAddressRequest {
    pub address: String,
}

#[derive(Serialize)]
pub struct WatchSparkAddressResponse {
    pub partial_address: String,
}

#[derive(Deserialize)]
pub struct WatchRunesAddressRequest {
    pub address: String,
}

#[derive(Serialize)]
pub struct WatchRunesAddressResponse {
    pub partial_address: String,
}

#[derive(Deserialize)]
pub struct GetRound1PackageRequest {
    pub metadata: String,
}

#[derive(Serialize)]
pub struct GetRound1PackageResponse {
    pub round_1_package: String,
}

#[derive(Deserialize)]
pub struct GetRound2PackageRequest {
    pub metadata: String,
}

#[derive(Serialize)]
pub struct GetRound2PackageResponse {
    pub round_2_package: String,
}

#[derive(Deserialize)]
pub struct GetRound3PackageRequest {
    pub metadata: String,
}

#[derive(Serialize)]
pub struct GetRound3PackageResponse {
    pub final_key_package: String,
}