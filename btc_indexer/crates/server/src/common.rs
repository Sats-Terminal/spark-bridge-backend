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
