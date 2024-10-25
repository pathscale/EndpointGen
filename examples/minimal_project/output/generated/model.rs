use endpoint_libs::libs::error_code::ErrorCode;
use endpoint_libs::libs::types::*;
use endpoint_libs::libs::ws::*;
use num_derive::FromPrimitive;
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
#[postgres(name = "enum_Endpoint")]
pub enum EnumEndpoint {
    ///
    #[postgres(name = "LatencyGet")]
    LatencyGet = 110,
    ///
    #[postgres(name = "test1")]
    test1 = 120,
}

impl EnumEndpoint {
    pub fn schema(&self) -> endpoint_libs::model::EndpointSchema {
        let schema = match self {
            Self::LatencyGet => LatencyGetRequest::SCHEMA,
            Self::Test1 => Test1Request::SCHEMA,
        };
        serde_json::from_str(schema).unwrap()
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ErrorXxx;
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
pub struct Latency {
    pub exchange_id: i32,
    pub measurement: Measurement,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LatencyGetRequest {
    #[serde(default)]
    pub exchange: Option<i32>,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LatencyGetResponse {
    pub latencies: Vec<Latency>,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Measurement {
    pub min: f64,
    pub avg: f64,
    pub max: f64,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Test1 {
    pub exchange_id: i32,
    pub measurement: Measurement,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct test1Request {
    pub test1: Vec<Test1>,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct test1Response;
impl WsRequest for LatencyGetRequest {
    type Response = LatencyGetResponse;
    const METHOD_ID: u32 = 110;
    const SCHEMA: &'static str = r#"{
  "name": "LatencyGet",
  "code": 110,
  "parameters": [
    {
      "name": "exchange",
      "ty": {
        "Optional": "Int"
      }
    }
  ],
  "returns": [
    {
      "name": "latencies",
      "ty": {
        "DataTable": {
          "name": "Latency",
          "fields": [
            {
              "name": "exchange_id",
              "ty": "Int"
            },
            {
              "name": "measurement",
              "ty": {
                "Struct": {
                  "name": "Measurement",
                  "fields": [
                    {
                      "name": "min",
                      "ty": "Numeric"
                    },
                    {
                      "name": "avg",
                      "ty": "Numeric"
                    },
                    {
                      "name": "max",
                      "ty": "Numeric"
                    }
                  ]
                }
              }
            }
          ]
        }
      }
    }
  ],
  "stream_response": null,
  "description": "",
  "json_schema": null
}"#;
}
impl WsResponse for LatencyGetResponse {
    type Request = LatencyGetRequest;
}

impl WsRequest for Test1Request {
    type Response = Test1Response;
    const METHOD_ID: u32 = 120;
    const SCHEMA: &'static str = r#"{
  "name": "test1",
  "code": 120,
  "parameters": [
    {
      "name": "test1",
      "ty": {
        "DataTable": {
          "name": "Test1",
          "fields": [
            {
              "name": "exchange_id",
              "ty": "Int"
            },
            {
              "name": "measurement",
              "ty": {
                "Struct": {
                  "name": "Measurement",
                  "fields": [
                    {
                      "name": "min",
                      "ty": "Numeric"
                    },
                    {
                      "name": "avg",
                      "ty": "Numeric"
                    },
                    {
                      "name": "max",
                      "ty": "Numeric"
                    }
                  ]
                }
              }
            }
          ]
        }
      }
    }
  ],
  "returns": [],
  "stream_response": null,
  "description": "",
  "json_schema": null
}"#;
}
impl WsResponse for Test1Response {
    type Request = Test1Request;
}
