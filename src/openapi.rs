//! OpenAPI 3.1 emission.
//!
//! **This document is a projection for tooling, not a description of a running
//! HTTP surface.** The server speaks WebSocket, carrying `{method, seq, params}`
//! frames; there are no URLs. The paths here are synthesized so that OpenAPI
//! tooling — client generators, doc renderers, OpenAPI→MCP bridges — has
//! something to chew on. Generating an HTTP client from this and pointing it at
//! the server will not work. The AsyncAPI document (see [`crate::asyncapi`]) is
//! the authoritative description of the wire protocol.
//!
//! That warning is repeated in the emitted `info.description` and in
//! `docs/openapi-README.md`, because an undocumented synthetic path map is worse
//! than no document at all: it looks usable.
//!
//! Conventions (PLAN-2.1 §3.1), which mirror the RON namespace `(service_name,
//! service_id)`:
//!
//! ```text
//! POST /{serviceName}/{endpoint_snake_name}
//!   operationId: {serviceName}_{endpoint_snake_name}
//!   tags:        [{serviceName}]
//! ```

use std::collections::BTreeMap;

use convert_case::{Case, Casing};
use endpoint_libs::model::{EndpointSchema, SchemaComponents, TypeRegistry, apply_meta};
use eyre::{Context, Result};
use serde_json::{Value, json};

use crate::definitions::{ErrorCodeSchema, GenService};
use crate::docs::Data;

/// The security scheme name used for every operation.
const SESSION_TOKEN_SCHEME: &str = "sessionToken";

/// Builds the document. Separated from writing so tests can assert on the value
/// without touching a filesystem.
pub fn build_openapi(data: &Data, public_only: bool) -> Result<Value> {
    let registry = build_registry(data);
    let services = visible_services(data, public_only);

    let all_endpoints: Vec<EndpointSchema> = services
        .iter()
        .flat_map(|s| s.endpoints.iter().map(|e| e.schema.clone()))
        .collect();

    let components = SchemaComponents::collect(&all_endpoints, &registry)
        .wrap_err("collecting shared schema components for the OpenAPI document")?;

    let mut paths = serde_json::Map::new();
    let mut tags = Vec::new();

    for service in &services {
        tags.push(json!({
            "name": service.name,
            "description": format!("Endpoints of the `{}` service (service id {}).", service.name, service.id),
        }));

        for element in &service.endpoints {
            let schema = &element.schema;
            let path = operation_path(&service.name, &schema.name);
            let operation = build_operation(
                service,
                schema,
                element.frontend_facing,
                &components,
                &registry,
                &data.error_codes,
            )
            .with_context(|| format!("endpoint {} ({})", schema.name, schema.code))?;

            paths.insert(path, json!({ "post": operation }));
        }
    }

    let mut schemas: BTreeMap<String, Value> = components.schemas.clone();
    schemas.insert("ErrorEnvelope".into(), error_envelope_schema());

    Ok(json!({
        "openapi": "3.1.0",
        "info": {
            "title": document_title(data),
            "version": "1.0.0",
            "description": INFO_DESCRIPTION,
        },
        "tags": tags,
        "paths": Value::Object(paths),
        "components": {
            "schemas": schemas,
            "securitySchemes": {
                SESSION_TOKEN_SCHEME: {
                    "type": "apiKey",
                    "in": "header",
                    "name": "Sec-WebSocket-Protocol",
                    "description": "Auth token passed as a WebSocket subprotocol; see AuthController.",
                }
            },
        },
    }))
}

/// Writes `docs/openapi.json`.
pub fn gen_openapi(data: &Data, public_only: bool) -> Result<()> {
    let docs_dir = data.project_root.join("docs");
    std::fs::create_dir_all(&docs_dir)?;

    let document = build_openapi(data, public_only)?;
    let filename = docs_dir.join("openapi.json");
    let file = std::fs::File::create(&filename)
        .with_context(|| format!("Failed to create OpenAPI file: {}", filename.display()))?;
    serde_json::to_writer_pretty(file, &document)?;
    Ok(())
}

/// The synthetic-path warning. Non-negotiable — see the module docs.
const INFO_DESCRIPTION: &str = "\
PROJECTION FOR TOOLING — NOT A SERVABLE HTTP API.

This document is generated from declarative RON endpoint definitions
(endpoint-libs / endpoint-gen). The production transport is a persistent
WebSocket carrying {method, seq, params} frames, plus MCP JSON-RPC 2.0 on the
same socket. There are no HTTP paths; the ones below are synthesized as
/{serviceName}/{endpoint_snake_name} so that OpenAPI tooling has a stable
handle on each operation.

Generating an HTTP client from this document and pointing it at the server
will NOT work. For an authoritative description of the wire protocol, use the
AsyncAPI document (docs/asyncapi.json).

Vendor extensions: x-endpoint-code (the wire method code), x-roles (RBAC roles
required), x-frontend-facing, x-stream-response, x-error-codes.";

/// `POST /{serviceName}/{endpoint_snake_name}`.
///
/// Endpoint names are only conventionally unique across services — the RON
/// namespace is `(service_name, service_id)` — so the path must carry the
/// service. A flat `/rpc/{Name}` scheme collides the moment two services reuse
/// a name, and `/rpc/` carries no information.
fn operation_path(service_name: &str, endpoint_name: &str) -> String {
    format!("/{}/{}", service_name, endpoint_name.to_case(Case::Snake))
}

fn operation_id(service_name: &str, endpoint_name: &str) -> String {
    format!("{}_{}", service_name, endpoint_name.to_case(Case::Snake))
}

fn document_title(data: &Data) -> String {
    data.project_root
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "API".into())
}

fn build_registry(data: &Data) -> TypeRegistry {
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
/// entirely so the document has no empty tags.
fn visible_services(data: &Data, public_only: bool) -> Vec<GenService> {
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

#[allow(clippy::too_many_arguments)]
fn build_operation(
    service: &GenService,
    schema: &EndpointSchema,
    frontend_facing: bool,
    components: &SchemaComponents,
    registry: &TypeRegistry,
    error_codes: &[ErrorCodeSchema],
) -> Result<Value> {
    let mut operation = serde_json::Map::new();

    operation.insert("operationId".into(), json!(operation_id(&service.name, &schema.name)));
    operation.insert("tags".into(), json!([service.name]));

    // `summary` is the first line, `description` the whole thing. Renderers show
    // summary in the operation list and description on the detail page.
    if !schema.description.is_empty() {
        let summary = schema.description.lines().next().unwrap_or_default().trim();
        if !summary.is_empty() {
            operation.insert("summary".into(), json!(summary));
        }
        operation.insert("description".into(), json!(schema.description));
    }

    operation.insert("x-endpoint-code".into(), json!(schema.code));
    operation.insert("x-frontend-facing".into(), json!(frontend_facing));
    if !schema.roles.is_empty() {
        operation.insert("x-roles".into(), json!(schema.roles));
    }

    if schema.stream_response.is_some() {
        operation.insert("x-stream-response".into(), json!(true));
        let note = "This endpoint streams: the server may send multiple response \
                    frames for one request. That has no HTTP equivalent and is not \
                    represented in the responses below — see the AsyncAPI document.";
        let described = match operation.get("description") {
            Some(Value::String(existing)) => format!("{existing}\n\n{note}"),
            _ => note.to_string(),
        };
        operation.insert("description".into(), json!(described));
    }

    operation.insert(
        "security".into(),
        json!([{ SESSION_TOKEN_SCHEME: Vec::<String>::new() }]),
    );

    if !schema.parameters.is_empty() {
        let request = components.request_schema(schema, registry)?;
        operation.insert(
            "requestBody".into(),
            json!({
                "required": true,
                "content": { "application/json": { "schema": request } },
            }),
        );
    }

    operation.insert(
        "responses".into(),
        build_responses(schema, components, registry, error_codes)?,
    );

    let mut operation = Value::Object(operation);
    apply_meta(&mut operation, &schema.meta, &format!("endpoint {}", schema.name))?;
    Ok(operation)
}

/// `200` plus a `default` error response.
///
/// The wire protocol has no status codes, so inventing per-error HTTP statuses
/// would be fiction. The envelope is the contract; `x-error-codes` lists what
/// this endpoint may return, resolved against the global catalog.
fn build_responses(
    schema: &EndpointSchema,
    components: &SchemaComponents,
    registry: &TypeRegistry,
    error_codes: &[ErrorCodeSchema],
) -> Result<Value> {
    let response_schema = components.response_schema(schema, registry)?;

    let mut error_response = json!({
        "description": "Error envelope. The wire protocol has no status codes; \
                        failures arrive as this object.",
        "content": {
            "application/json": {
                "schema": { "$ref": "#/components/schemas/ErrorEnvelope" }
            }
        },
    });

    if !schema.errors.is_empty() {
        let catalog: BTreeMap<&str, &ErrorCodeSchema> = error_codes.iter().map(|c| (c.name.as_str(), c)).collect();

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

        error_response["x-error-codes"] = Value::Array(listed);
    }

    Ok(json!({
        "200": {
            "description": "Success.",
            "content": { "application/json": { "schema": response_schema } },
        },
        "default": error_response,
    }))
}

/// The standard error envelope: code, message, params.
fn error_envelope_schema() -> Value {
    json!({
        "type": "object",
        "title": "ErrorEnvelope",
        "description": "Standard error payload returned in place of a success response.",
        "properties": {
            "code": { "type": "integer", "description": "Error code from the global catalog." },
            "message": { "type": "string" },
            "params": { "type": "object", "description": "Error-specific fields, if any." },
        },
        "required": ["code", "message"],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::definitions::{EndpointSchemaElement, RustGenConfig};
    use endpoint_libs::model::{EndpointErrorCodeRef, EndpointErrorSchema, Field, Type, collect_refs};
    use std::path::PathBuf;

    fn element(schema: EndpointSchema, frontend_facing: bool) -> EndpointSchemaElement {
        EndpointSchemaElement {
            frontend_facing,
            config: RustGenConfig::default(),
            schema,
        }
    }

    fn data_with(services: Vec<GenService>) -> Data {
        Data {
            project_root: PathBuf::from("/tmp/api.example.com"),
            output_dir: PathBuf::from("/tmp/api.example.com/generated"),
            services,
            enums: vec![],
            structs: vec![],
            error_codes: vec![ErrorCodeSchema::new("BadRequest", 400, "The request was malformed.")],
        }
    }

    fn sample_data() -> Data {
        let login = EndpointSchema::new(
            "UserLogin",
            10000,
            vec![
                Field::new("user_name", Type::String),
                Field::new("cursor", Type::Optional(Box::new(Type::String))),
            ],
            vec![Field::new("access_token", Type::String)],
        )
        .with_description("Logs a user in.\nReturns a session token.")
        .with_errors(vec![
            EndpointErrorSchema::new("PasswordTooShort", EndpointErrorCodeRef::new("BadRequest"))
                .with_message("Password too short"),
        ]);

        let internal = EndpointSchema::new("AdminPurge", 40000, vec![], vec![]).with_description("Purges everything.");

        data_with(vec![GenService::new(
            "userApi".into(),
            1,
            vec![element(login, true), element(internal, false)],
        )])
    }

    #[test]
    fn paths_and_operation_ids_follow_the_service_convention() {
        let doc = build_openapi(&sample_data(), false).unwrap();
        let op = &doc["paths"]["/userApi/user_login"]["post"];

        assert_eq!(op["operationId"], "userApi_user_login");
        assert_eq!(op["tags"], json!(["userApi"]));
        assert_eq!(op["x-endpoint-code"], 10000);
    }

    #[test]
    fn summary_is_the_first_line_description_is_the_whole_thing() {
        let doc = build_openapi(&sample_data(), false).unwrap();
        let op = &doc["paths"]["/userApi/user_login"]["post"];

        assert_eq!(op["summary"], "Logs a user in.");
        assert_eq!(op["description"], "Logs a user in.\nReturns a session token.");
    }

    #[test]
    fn optional_parameters_are_not_required() {
        let doc = build_openapi(&sample_data(), false).unwrap();
        let schema =
            &doc["paths"]["/userApi/user_login"]["post"]["requestBody"]["content"]["application/json"]["schema"];
        assert_eq!(schema["required"], json!(["userName"]));
    }

    #[test]
    fn errors_become_a_default_response_not_invented_statuses() {
        let doc = build_openapi(&sample_data(), false).unwrap();
        let responses = &doc["paths"]["/userApi/user_login"]["post"]["responses"];

        assert!(responses["200"].is_object());
        assert!(responses["default"].is_object());
        assert!(
            responses.get("400").is_none(),
            "must not invent per-error HTTP statuses"
        );

        let listed = &responses["default"]["x-error-codes"][0];
        assert_eq!(listed["name"], "PasswordTooShort");
        assert_eq!(listed["code"], "BadRequest");
        // Resolved against the global catalog.
        assert_eq!(listed["value"], 400);
        assert_eq!(listed["description"], "The request was malformed.");
    }

    #[test]
    fn public_only_drops_exactly_the_non_frontend_facing_operations() {
        let full = build_openapi(&sample_data(), false).unwrap();
        assert!(full["paths"]["/userApi/admin_purge"].is_object());

        let public = build_openapi(&sample_data(), true).unwrap();
        assert!(public["paths"]["/userApi/user_login"].is_object());
        assert!(
            public["paths"].get("/userApi/admin_purge").is_none(),
            "public-only document must drop internal operations"
        );
    }

    #[test]
    fn every_ref_resolves_and_no_defs_survive() {
        let registry_struct = Type::struct_(
            "Wallet",
            vec![
                Field::new("id", Type::Int64),
                Field::new("owner", Type::StructRef("Wallet".into())),
            ],
        );
        let mut data = data_with(vec![GenService::new(
            "walletApi".into(),
            2,
            vec![element(
                EndpointSchema::new(
                    "GetWallet",
                    20000,
                    vec![],
                    vec![Field::new("wallet", Type::StructRef("Wallet".into()))],
                )
                .with_description("Gets a wallet."),
                true,
            )],
        )]);
        data.structs = vec![crate::definitions::StructElement {
            config: RustGenConfig::default(),
            inner: registry_struct,
        }];

        let doc = build_openapi(&data, false).unwrap();

        let mut refs = vec![];
        collect_refs(&doc, &mut refs);
        assert!(!refs.is_empty());

        let schemas = doc["components"]["schemas"].as_object().unwrap();
        for target in &refs {
            let name = target
                .strip_prefix("#/components/schemas/")
                .unwrap_or_else(|| panic!("ref {target} is not a components ref"));
            assert!(schemas.contains_key(name), "dangling ref: {target}");
        }

        let serialised = serde_json::to_string(&doc).unwrap();
        assert!(!serialised.contains("$defs"), "no $defs may survive into the document");
    }

    #[test]
    fn the_synthetic_path_warning_is_present() {
        let doc = build_openapi(&sample_data(), false).unwrap();
        let description = doc["info"]["description"].as_str().unwrap();
        assert!(description.contains("NOT A SERVABLE HTTP API"));
        assert!(description.contains("AsyncAPI"));
    }

    #[test]
    fn every_operation_carries_security() {
        let doc = build_openapi(&sample_data(), false).unwrap();
        let op = &doc["paths"]["/userApi/user_login"]["post"];
        assert_eq!(op["security"], json!([{ "sessionToken": [] }]));
        assert!(doc["components"]["securitySchemes"]["sessionToken"].is_object());
    }
}
