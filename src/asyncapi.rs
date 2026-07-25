//! AsyncAPI 3.0 emission.
//!
//! **This is the document that actually describes the protocol.** The OpenAPI
//! document ([`crate::openapi`]) is a projection for tooling with synthesized
//! paths; this one describes what really goes over the wire: a persistent
//! WebSocket carrying `{method, seq, params}` request frames and matching
//! response frames, with MCP JSON-RPC 2.0 sharing the same socket.
//!
//! One merged document, one channel per service — matching the merged OpenAPI
//! document. The services share a transport and are distinguished by the
//! `method` code inside the envelope, not by endpoint, so splitting per service
//! would misrepresent the protocol *and* make the shared-components equality
//! test (§4.2) unsatisfiable.

use endpoint_libs::model::{EndpointSchema, SchemaComponents, TypeRegistry, apply_meta};
use eyre::{Context, Result};
use serde_json::{Value, json};

use crate::definitions::{ErrorCodeSchema, GenService};
use crate::docs::Data;
use crate::spec_common::{
    ERROR_ENVELOPE, build_registry, collect_components, document_schemas, document_title, error_code_list,
    visible_services,
};

/// Builds the document. Separated from writing so tests can assert on the value.
pub fn build_asyncapi(data: &Data, public_only: bool) -> Result<Value> {
    let registry = build_registry(data);
    let services = visible_services(data, public_only);
    let components = collect_components(&services, &registry)?;

    let mut channels = serde_json::Map::new();
    let mut operations = serde_json::Map::new();
    let mut messages = serde_json::Map::new();

    // The generic envelope messages, shared by every channel.
    messages.insert("Request".into(), request_envelope_message(&services));
    messages.insert("Response".into(), response_envelope_message());
    messages.insert("Error".into(), error_envelope_message());
    messages.insert("McpJsonRpc".into(), mcp_message());

    for service in &services {
        let channel_name = service.name.clone();

        channels.insert(
            channel_name.clone(),
            json!({
                "address": "/",
                "title": format!("{} WebSocket channel", service.name),
                "description": format!(
                    "Requests for the `{}` service (service id {}). All services share one \
                     socket; the `method` code in the request envelope selects the endpoint.",
                    service.name, service.id
                ),
                "bindings": {
                    "ws": {
                        "bindingVersion": "0.1.0",
                        "method": "GET",
                        "headers": {
                            "type": "object",
                            "properties": {
                                "Sec-WebSocket-Protocol": {
                                    "type": "string",
                                    "description": "Auth token, passed as a WebSocket subprotocol.",
                                }
                            }
                        }
                    }
                },
                "messages": {
                    "Request": { "$ref": "#/components/messages/Request" },
                    "Response": { "$ref": "#/components/messages/Response" },
                    "Error": { "$ref": "#/components/messages/Error" },
                    "McpJsonRpc": { "$ref": "#/components/messages/McpJsonRpc" },
                },
            }),
        );

        operations.insert(
            format!("{}_sendRequest", service.name),
            json!({
                "action": "send",
                "channel": { "$ref": format!("#/channels/{channel_name}") },
                "title": format!("Send a request to {}", service.name),
                "messages": [
                    { "$ref": format!("#/channels/{channel_name}/messages/Request") },
                    { "$ref": format!("#/channels/{channel_name}/messages/McpJsonRpc") },
                ],
            }),
        );
        operations.insert(
            format!("{}_receiveResponse", service.name),
            json!({
                "action": "receive",
                "channel": { "$ref": format!("#/channels/{channel_name}") },
                "title": format!("Receive a response from {}", service.name),
                "messages": [
                    { "$ref": format!("#/channels/{channel_name}/messages/Response") },
                    { "$ref": format!("#/channels/{channel_name}/messages/Error") },
                ],
            }),
        );

        // Per-endpoint message pairs.
        for element in &service.endpoints {
            let schema = &element.schema;
            let (request, response) = endpoint_messages(
                service,
                schema,
                element.frontend_facing,
                &components,
                &registry,
                &data.error_codes,
            )
            .with_context(|| format!("endpoint {} ({})", schema.name, schema.code))?;

            messages.insert(format!("{}Request", schema.name), request);
            messages.insert(format!("{}Response", schema.name), response);
        }
    }

    channels.insert("framedJson".into(), framed_json_channel());

    Ok(json!({
        "asyncapi": "3.0.0",
        "info": {
            "title": document_title(data),
            "version": "1.0.0",
            "description": INFO_DESCRIPTION,
        },
        "channels": Value::Object(channels),
        "operations": Value::Object(operations),
        "components": {
            "schemas": document_schemas(&components),
            "messages": Value::Object(messages),
        },
    }))
}

/// Writes `docs/asyncapi.json`.
pub fn gen_asyncapi(data: &Data, public_only: bool) -> Result<()> {
    let docs_dir = data.project_root.join("docs");
    std::fs::create_dir_all(&docs_dir)?;

    let document = build_asyncapi(data, public_only)?;
    let filename = docs_dir.join("asyncapi.json");
    let file = std::fs::File::create(&filename)
        .with_context(|| format!("Failed to create AsyncAPI file: {}", filename.display()))?;
    serde_json::to_writer_pretty(file, &document)?;
    Ok(())
}

const INFO_DESCRIPTION: &str = "\
Authoritative description of the wire protocol.

Clients hold one persistent WebSocket and send request frames shaped
{method, seq, params}, where `method` is the endpoint code and `seq` correlates
the response. Responses carry the same `seq`. MCP JSON-RPC 2.0 traffic shares
the same socket.

Auth is a token passed as the WebSocket subprotocol (Sec-WebSocket-Protocol).

The companion OpenAPI document (docs/openapi.json) is a projection for HTTP
tooling with synthesized paths; it is not servable. This document is the one
to implement against.

A second channel, `framedJson`, documents the length-delimited framing used for
non-WebSocket local transports (Unix sockets, named pipes) under x-framing.";

/// The request envelope: `{ method, seq, params }`.
///
/// `params` is a `oneOf` over every endpoint's parameter schema. AsyncAPI 3.0
/// permits a discriminator only on a property of the payload itself, and
/// `method` is an integer code rather than a schema name, so a standard
/// `discriminator` cannot express the mapping. `x-method-map` carries it
/// instead: code → message name.
fn request_envelope_message(services: &[GenService]) -> Value {
    let mut variants = Vec::new();
    let mut method_map = serde_json::Map::new();

    for service in services {
        for element in &service.endpoints {
            let name = &element.schema.name;
            variants.push(json!({
                "$ref": format!("#/components/messages/{name}Request/payload")
            }));
            method_map.insert(element.schema.code.to_string(), json!(format!("{name}Request")));
        }
    }

    json!({
        "name": "Request",
        "title": "Request envelope",
        "summary": "A client-to-server call. `method` selects the endpoint; `seq` correlates the response.",
        "contentType": "application/json",
        "payload": {
            "type": "object",
            "properties": {
                "method": { "type": "integer", "description": "Endpoint code." },
                "seq": { "type": "integer", "description": "Client-chosen correlation id." },
                "params": {
                    "description": "Endpoint parameters; shape selected by `method`.",
                    "oneOf": variants,
                },
            },
            "required": ["method", "seq", "params"],
        },
        "x-method-map": Value::Object(method_map),
    })
}

fn response_envelope_message() -> Value {
    json!({
        "name": "Response",
        "title": "Response envelope",
        "summary": "A successful server-to-client reply, correlated by `seq`.",
        "contentType": "application/json",
        "payload": {
            "type": "object",
            "properties": {
                "method": { "type": "integer", "description": "Echoes the request's endpoint code." },
                "seq": { "type": "integer", "description": "Echoes the request's correlation id." },
                "params": { "description": "Endpoint return value; shape selected by `method`." },
            },
            "required": ["method", "seq"],
        },
    })
}

fn error_envelope_message() -> Value {
    json!({
        "name": "Error",
        "title": "Error envelope",
        "summary": "A failed call. The protocol has no status codes; this object is the contract.",
        "contentType": "application/json",
        "payload": { "$ref": format!("#/components/schemas/{ERROR_ENVELOPE}") },
    })
}

fn mcp_message() -> Value {
    json!({
        "name": "McpJsonRpc",
        "title": "MCP JSON-RPC 2.0",
        "summary": "Model Context Protocol traffic, sharing the same socket as the RPC frames.",
        "contentType": "application/json",
        "payload": {
            "type": "object",
            "properties": {
                "jsonrpc": { "const": "2.0" },
                "id": { "description": "Request id; absent for notifications." },
                "method": { "type": "string" },
                "params": { "type": "object" },
            },
            "required": ["jsonrpc"],
        },
    })
}

/// Per-endpoint request/response message pair.
fn endpoint_messages(
    service: &GenService,
    schema: &EndpointSchema,
    frontend_facing: bool,
    components: &SchemaComponents,
    registry: &TypeRegistry,
    error_codes: &[ErrorCodeSchema],
) -> Result<(Value, Value)> {
    let mut request = serde_json::Map::new();
    request.insert("name".into(), json!(format!("{}Request", schema.name)));
    request.insert("title".into(), json!(schema.name.clone()));
    request.insert("contentType".into(), json!("application/json"));
    if !schema.description.is_empty() {
        request.insert("summary".into(), json!(first_line(&schema.description)));
        request.insert("description".into(), json!(schema.description));
    }
    request.insert("x-endpoint-code".into(), json!(schema.code));
    request.insert("x-service".into(), json!(service.name));
    request.insert("x-frontend-facing".into(), json!(frontend_facing));
    if !schema.roles.is_empty() {
        request.insert("x-roles".into(), json!(schema.roles));
    }
    if let Some(errors) = error_code_list(schema, error_codes) {
        request.insert("x-error-codes".into(), errors);
    }
    request.insert("payload".into(), components.request_schema(schema, registry)?);

    let mut request = Value::Object(request);
    apply_meta(&mut request, &schema.meta, &format!("endpoint {}", schema.name))?;

    let mut response = serde_json::Map::new();
    response.insert("name".into(), json!(format!("{}Response", schema.name)));
    response.insert("title".into(), json!(format!("{} result", schema.name)));
    response.insert("contentType".into(), json!("application/json"));
    response.insert("x-endpoint-code".into(), json!(schema.code));
    response.insert("x-service".into(), json!(service.name));
    if schema.stream_response.is_some() {
        response.insert("x-stream-response".into(), json!(true));
        response.insert(
            "summary".into(),
            json!("Streaming: the server may send multiple response frames for one request."),
        );
    }
    response.insert("payload".into(), components.response_schema(schema, registry)?);

    Ok((request, Value::Object(response)))
}

fn first_line(text: &str) -> String {
    text.lines().next().unwrap_or_default().trim().to_string()
}

/// The length-delimited framing used by non-WebSocket local transports.
///
/// This is the only machine-readable description of that format anywhere, so a
/// non-Rust peer implementing the local transport should be able to write a
/// codec from `x-framing` alone. Kept in sync with
/// `endpoint-libs/src/libs/ws/transport/framed.rs`.
fn framed_json_channel() -> Value {
    json!({
        "address": "/",
        "title": "Framed local transport",
        "description": "Length-delimited framing for non-WebSocket byte streams (Unix sockets, \
                        named pipes, inherited socketpairs). Same messages as the WebSocket \
                        channels; only the framing differs.",
        "messages": {
            "Request": { "$ref": "#/components/messages/Request" },
            "Response": { "$ref": "#/components/messages/Response" },
            "Error": { "$ref": "#/components/messages/Error" },
        },
        "x-framing": {
            "layout": "u32 BE length | u8 kind | payload",
            "lengthField": {
                "bytes": 4,
                "endianness": "big",
                "counts": "the kind byte plus the payload — i.e. the whole rest of the frame",
            },
            "kindField": {
                "bytes": 1,
                "values": {
                    "0": "Text — payload is UTF-8",
                    "1": "Binary",
                    "2": "Ping",
                    "3": "Pong",
                    "4": "Close — payload is u16 BE code followed by a UTF-8 reason",
                },
            },
            "maxFrameBytes": 16 * 1024 * 1024,
            "notes": "Wire-compatible with tokio_util::codec::LengthDelimitedCodec's u32 BE \
                      prefix, but NOT with tokio_serde: the kind byte means the payload is not \
                      a bare serde value.",
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::definitions::{EndpointSchemaElement, RustGenConfig};
    use endpoint_libs::model::{Field, Type, collect_refs};
    use std::path::PathBuf;

    fn element(schema: EndpointSchema, frontend_facing: bool) -> EndpointSchemaElement {
        EndpointSchemaElement {
            frontend_facing,
            config: RustGenConfig::default(),
            schema,
        }
    }

    fn sample_data() -> Data {
        let login = EndpointSchema::new(
            "UserLogin",
            10000,
            vec![Field::new("user_name", Type::String)],
            vec![Field::new("access_token", Type::String)],
        )
        .with_description("Logs a user in.");

        Data {
            project_root: PathBuf::from("/tmp/api.example.com"),
            output_dir: PathBuf::from("/tmp/api.example.com/generated"),
            services: vec![GenService::new("userApi".into(), 1, vec![element(login, true)])],
            enums: vec![],
            structs: vec![],
            error_codes: vec![],
        }
    }

    #[test]
    fn one_channel_per_service_plus_the_framed_transport() {
        let doc = build_asyncapi(&sample_data(), false).unwrap();
        let channels = doc["channels"].as_object().unwrap();

        assert!(channels.contains_key("userApi"));
        assert!(channels.contains_key("framedJson"));
    }

    #[test]
    fn send_and_receive_operations_exist_per_service() {
        let doc = build_asyncapi(&sample_data(), false).unwrap();
        let ops = &doc["operations"];

        assert_eq!(ops["userApi_sendRequest"]["action"], "send");
        assert_eq!(ops["userApi_receiveResponse"]["action"], "receive");
    }

    #[test]
    fn request_envelope_carries_the_method_map() {
        let doc = build_asyncapi(&sample_data(), false).unwrap();
        let envelope = &doc["components"]["messages"]["Request"];

        assert_eq!(envelope["payload"]["required"], json!(["method", "seq", "params"]));
        // Code 10000 maps to the per-endpoint message.
        assert_eq!(envelope["x-method-map"]["10000"], "UserLoginRequest");
    }

    #[test]
    fn per_endpoint_message_pairs_are_emitted() {
        let doc = build_asyncapi(&sample_data(), false).unwrap();
        let messages = &doc["components"]["messages"];

        assert_eq!(messages["UserLoginRequest"]["x-endpoint-code"], 10000);
        assert_eq!(messages["UserLoginRequest"]["x-service"], "userApi");
        assert_eq!(messages["UserLoginResponse"]["x-endpoint-code"], 10000);
    }

    #[test]
    fn framing_is_reconstructible_from_x_framing_alone() {
        let doc = build_asyncapi(&sample_data(), false).unwrap();
        let framing = &doc["channels"]["framedJson"]["x-framing"];

        assert_eq!(framing["layout"], "u32 BE length | u8 kind | payload");
        assert_eq!(framing["lengthField"]["bytes"], 4);
        assert_eq!(framing["lengthField"]["endianness"], "big");
        assert_eq!(framing["kindField"]["values"]["0"], "Text — payload is UTF-8");
        assert_eq!(framing["maxFrameBytes"], 16 * 1024 * 1024);
    }

    #[test]
    fn shared_components_match_the_openapi_document() {
        // PLAN-2.1 section 4.2: both documents must reference identical schema
        // objects. Guaranteed by construction (spec_common::document_schemas),
        // asserted here so a divergent refactor fails.
        let data = sample_data();
        let openapi = crate::openapi::build_openapi(&data, false).unwrap();
        let asyncapi = build_asyncapi(&data, false).unwrap();

        assert_eq!(openapi["components"]["schemas"], asyncapi["components"]["schemas"]);
    }

    #[test]
    fn every_ref_resolves_and_no_defs_survive() {
        let doc = build_asyncapi(&sample_data(), false).unwrap();

        let mut refs = vec![];
        collect_refs(&doc, &mut refs);
        assert!(!refs.is_empty());

        for target in &refs {
            let resolves = target.starts_with("#/components/schemas/")
                || target.starts_with("#/components/messages/")
                || target.starts_with("#/channels/");
            assert!(resolves, "unexpected ref target: {target}");
        }

        // Schema refs must land in components.schemas.
        let schemas = doc["components"]["schemas"].as_object().unwrap();
        for target in &refs {
            if let Some(name) = target.strip_prefix("#/components/schemas/") {
                assert!(schemas.contains_key(name), "dangling schema ref: {target}");
            }
        }

        let serialised = serde_json::to_string(&doc).unwrap();
        assert!(!serialised.contains("$defs"), "no $defs may survive");
    }
}
