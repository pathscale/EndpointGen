//! Pieces shared by the OpenAPI and AsyncAPI emitters.
//!
//! Both documents must describe the same API, so anything that would otherwise
//! be written twice lives here. In particular [`document_schemas`] is what makes
//! `openapi.components.schemas == asyncapi.components.schemas` true by
//! construction rather than by coincidence — there is a test asserting it, and
//! the only honest way to keep that passing is to have one function build it.

use std::collections::BTreeMap;

use endpoint_libs::model::{EndpointSchema, SchemaComponents, TypeRegistry};
use eyre::{Result, WrapErr};
use serde_json::{Value, json};

use crate::definitions::GenService;
use crate::docs::Data;

/// Name of the shared error payload schema, referenced from both documents.
pub const ERROR_ENVELOPE: &str = "ErrorEnvelope";

/// Builds the type registry over every struct, enum and endpoint in the project.
pub fn build_registry(data: &Data) -> TypeRegistry {
    let mut registry = TypeRegistry::new();
    registry.add_all(crate::rust::shared_type_definitions(data).iter());
    for service in &data.services {
        for endpoint in &service.endpoints {
            registry.add_endpoint(&endpoint.schema);
        }
    }
    registry
}

/// Services filtered by `--public-only`.
///
/// Filtering is per endpoint, not per service: a service with a mix keeps only
/// its frontend-facing operations. A service left with nothing is dropped
/// entirely, so neither document carries an empty channel or tag.
pub fn visible_services(data: &Data, public_only: bool) -> Vec<GenService> {
    data.services
        .iter()
        .filter_map(|service| {
            let endpoints: Vec<_> = service
                .endpoints
                .iter()
                .filter(|e| !public_only || e.frontend_facing)
                .cloned()
                .collect();
            if endpoints.is_empty() {
                return None;
            }
            let mut service = service.clone();
            service.endpoints = endpoints;
            Some(service)
        })
        .collect()
}

/// Every endpoint schema across the given services, in document order.
pub fn all_endpoints(services: &[GenService]) -> Vec<EndpointSchema> {
    services
        .iter()
        .flat_map(|s| s.endpoints.iter().map(|e| e.schema.clone()))
        .collect()
}

/// Collects shared components for a set of services.
pub fn collect_components(services: &[GenService], registry: &TypeRegistry) -> Result<SchemaComponents> {
    let endpoints = all_endpoints(services);
    SchemaComponents::collect(&endpoints, registry).wrap_err("collecting shared schema components")
}

/// `components.schemas` for either document: the collected definitions plus the
/// shared error envelope.
///
/// Both emitters call this so the two documents' schema sections are identical
/// by construction.
pub fn document_schemas(components: &SchemaComponents) -> BTreeMap<String, Value> {
    let mut schemas = components.schemas.clone();
    schemas.insert(ERROR_ENVELOPE.into(), error_envelope_schema());
    schemas
}

/// The standard error payload: code, message, params.
///
/// The wire protocol has no status codes, so this envelope *is* the error
/// contract in both documents.
pub fn error_envelope_schema() -> Value {
    json!({
        "type": "object",
        "title": ERROR_ENVELOPE,
        "description": "Standard error payload returned in place of a success response.",
        "properties": {
            "code": { "type": "integer", "description": "Error code from the global catalog." },
            "message": { "type": "string" },
            "params": { "type": "object", "description": "Error-specific fields, if any." },
        },
        "required": ["code", "message"],
    })
}

/// `x-error-codes` for an endpoint: what it may return, resolved against the
/// global catalog for numeric values and descriptions.
pub fn error_code_list(schema: &EndpointSchema, error_codes: &[crate::definitions::ErrorCodeSchema]) -> Option<Value> {
    if schema.errors.is_empty() {
        return None;
    }
    let catalog: BTreeMap<&str, &crate::definitions::ErrorCodeSchema> =
        error_codes.iter().map(|c| (c.name.as_str(), c)).collect();

    let listed: Vec<Value> = schema
        .errors
        .iter()
        .map(|error| {
            let variant = error.code.variant();
            let mut entry = serde_json::Map::new();
            entry.insert("name".into(), json!(error.name));
            entry.insert("code".into(), json!(variant));
            if let Some(known) = catalog.get(variant) {
                entry.insert("value".into(), json!(known.code));
                if !known.description.is_empty() {
                    entry.insert("description".into(), json!(known.description));
                }
            }
            if !error.message.is_empty() {
                entry.insert("message".into(), json!(error.message));
            }
            Value::Object(entry)
        })
        .collect();

    Some(Value::Array(listed))
}

/// Document title, derived from the project directory name.
pub fn document_title(data: &Data) -> String {
    data.project_root
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "API".into())
}
