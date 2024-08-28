use crate::types::*;
use serde::*;
#[derive(Debug, Serialize, Deserialize)]
pub struct EndpointSchema {
    pub name: String,
    pub code: u32,
    pub parameters: Vec<Field>,
    pub returns: Vec<Field>,
    pub stream_response: Option<Type>,
    pub description: String,
    pub json_schema: serde_json::Value,
}

impl EndpointSchema {
    pub fn new(
        name: impl Into<String>,
        code: u32,
        parameters: Vec<Field>,
        returns: Vec<Field>,
    ) -> Self {
        Self {
            name: name.into(),
            code,
            parameters,
            returns,
            stream_response: None,
            description: "".to_string(),
            json_schema: Default::default(),
        }
    }
    pub fn with_stream_response_type(mut self, stream_response: Type) -> Self {
        self.stream_response = Some(stream_response);
        self
    }
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }
}