use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4};

use axum::{
    Router,
    extract::{Json, State},
    routing::post,
};
use btc_indexer_internals::indexer::BtcIndexer;
use config_parser::config::ServerConfig;
use persistent_storage::init::PersistentRepoShared;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::info;
use utoipa::{
    OpenApi, PartialSchema, ToSchema, openapi,
    openapi::{Object, SchemaFormat},
};
use utoipa_swagger_ui::SwaggerUi;

use crate::{AppState, error::ServerError};

#[derive(Serialize, ToSchema)]
#[schema(example = json!({ }))]
pub struct Empty {}

#[derive(Serialize, Deserialize, Debug)]
#[serde(transparent)]
pub struct SocketAddrWrapped(pub SocketAddr);

impl PartialSchema for SocketAddrWrapped {
    fn schema() -> openapi::RefOr<openapi::schema::Schema> {
        utoipa::openapi::ObjectBuilder::new()
            .schema_type(utoipa::openapi::schema::SchemaType::Type(openapi::schema::Type::String))
            .example(Some(json!(&SocketAddr::V4(
                (SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 8080))
            ))))
            .into()
    }
}

impl utoipa::ToSchema for SocketAddrWrapped {
    fn name() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed("SocketAddr")
    }
}
