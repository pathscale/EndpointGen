use lib::error_code::ErrorCode;
use lib::types::*;
use lib::ws::*;
use num_derive::FromPrimitive;
use rust_decimal::Decimal;
use serde::*;
use strum_macros::{Display, EnumString};
use tokio_postgres::types::*;

#[derive(
    Debug,
    Clone,
    Copy,
    ToSql,
    FromSql,
    Serialize,
    Deserialize,
    FromPrimitive,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    EnumString,
    Display,
    Hash,
)]
#[postgres(name = "enum_role")]
pub enum EnumRole {
    ///
    #[postgres(name = "guest")]
    Guest = 0,
    ///
    #[postgres(name = "user")]
    User = 1,
    ///
    #[postgres(name = "admin")]
    Admin = 2,
    ///
    #[postgres(name = "developer")]
    Developer = 3,
}
#[derive(
    Debug,
    Clone,
    Copy,
    ToSql,
    FromSql,
    Serialize,
    Deserialize,
    FromPrimitive,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    EnumString,
    Display,
    Hash,
)]
#[postgres(name = "enum_Endpoint")]
pub enum EnumEndpoint {
    ///
    #[postgres(name = "Authorize")]
    Authorize = 10030,
}

impl EnumEndpoint {
    pub fn schema(&self) -> ::endpoint_gen::model::EndpointSchema {
        let schema = match self {
            Self::Authorize => AuthorizeRequest::SCHEMA,
        };
        serde_json::from_str(schema).unwrap()
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ErrorXxx {}
#[derive(
    Debug,
    Clone,
    Copy,
    ToSql,
    FromSql,
    Serialize,
    Deserialize,
    FromPrimitive,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    EnumString,
    Display,
    Hash,
)]
#[postgres(name = "enum_ErrorCode")]
pub enum EnumErrorCode {
    /// None Please populate error_codes.json
    #[postgres(name = "Xxx")]
    Xxx = 0,
}

impl From<EnumErrorCode> for ErrorCode {
    fn from(e: EnumErrorCode) -> Self {
        ErrorCode::new(e as _)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizeRequest {
    pub username: String,
    pub token: uuid::Uuid,
    pub service: EnumService,
    pub device_id: String,
    pub device_os: String,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizeResponse {
    pub success: bool,
}
impl WsRequest for AuthorizeRequest {
    type Response = AuthorizeResponse;
    const METHOD_ID: u32 = 10030;
    const SCHEMA: &'static str = r#"{
  "name": "Authorize",
  "code": 10030,
  "parameters": [
    {
      "name": "username",
      "ty": "String"
    },
    {
      "name": "token",
      "ty": "UUID"
    },
    {
      "name": "service",
      "ty": {
        "EnumRef": "service"
      }
    },
    {
      "name": "device_id",
      "ty": "String"
    },
    {
      "name": "device_os",
      "ty": "String"
    }
  ],
  "returns": [
    {
      "name": "success",
      "ty": "Boolean"
    }
  ],
  "stream_response": null,
  "description": "",
  "json_schema": null
}"#;
}
impl WsResponse for AuthorizeResponse {
    type Request = AuthorizeRequest;
}
