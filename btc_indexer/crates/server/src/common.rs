use std::{
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
    str::FromStr,
};

use bitcoin::Txid;
use serde::{Deserialize, Serialize};
use serde_json::json;
use url::Url;
use utoipa::{PartialSchema, ToSchema, openapi};

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
            .examples(Some(json!(&SocketAddr::V4(SocketAddrV4::new(
                Ipv4Addr::new(127, 0, 0, 1),
                8080
            )))))
            .into()
    }
}

impl utoipa::ToSchema for SocketAddrWrapped {
    fn name() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed("SocketAddr")
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(transparent)]
pub struct TxIdWrapped(pub Txid);

impl PartialSchema for TxIdWrapped {
    fn schema() -> openapi::RefOr<openapi::schema::Schema> {
        utoipa::openapi::ObjectBuilder::new()
            .schema_type(utoipa::openapi::schema::SchemaType::Type(openapi::schema::Type::String))
            .examples(Some(json!(&TxIdWrapped(
                Txid::from_str("fb0c9ab881331ec7acdd85d79e3197dcaf3f95055af1703aeee87e0d853e81ec",).unwrap()
            ))))
            .into()
    }
}

impl utoipa::ToSchema for TxIdWrapped {
    fn name() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed("TransactionId")
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(transparent)]
pub struct UrlWrapped(pub Url);

impl PartialSchema for UrlWrapped {
    fn schema() -> openapi::RefOr<openapi::schema::Schema> {
        utoipa::openapi::ObjectBuilder::new()
            .schema_type(utoipa::openapi::schema::SchemaType::Type(openapi::schema::Type::String))
            .examples(Some(json!(&Url::from_str("localhost:8080").unwrap())))
            .into()
    }
}

impl utoipa::ToSchema for UrlWrapped {
    fn name() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed("Url")
    }
}
