use crate::definitions::{EnumElement, ErrorCodeSchema, GenService, StructElement};
use crate::rust::ToRust;
use crate::service::get_systemd_service;
use convert_case::{Case, Casing};
use endpoint_libs::model::{EndpointSchema, Service, Type};
use eyre::Context;
use itertools::Itertools;
use serde_json::json;
use std::fs::{File, create_dir_all};
use std::io::Write;
use std::path::{Path, PathBuf};

pub struct Data {
    /// Human-facing name of the project, used as the title of the emitted
    /// specification documents.
    ///
    /// Deliberately separate from `project_root`: `--check` regenerates into a
    /// scratch directory, and deriving the title from the output path would
    /// make every document differ from its committed copy on every check.
    /// Document identity is data, not an artefact of where it was written.
    pub project_name: String,
    pub project_root: PathBuf,
    pub output_dir: PathBuf,
    pub services: Vec<GenService>,
    pub enums: Vec<EnumElement>,
    pub structs: Vec<StructElement>,
    pub error_codes: Vec<ErrorCodeSchema>,
}

pub fn gen_services_docs(docs: &Data) -> eyre::Result<()> {
    let docs_filename = docs.project_root.join("docs").join("services.json");

    // Ensure the parent directories exist
    if let Some(parent) = docs_filename.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut docs_file = File::create(&docs_filename)
        .with_context(|| format!("Failed to create docs file: {}", docs_filename.display()))?;

    // Only write FE facing endpoints to the services.json file
    let services = docs
        .services
        .clone()
        .into_iter()
        .map(|service| {
            let fe_endpoints: Vec<EndpointSchema> = service
                .endpoints
                .into_iter()
                .filter(|endpoint| endpoint.frontend_facing)
                .collect();

            Service::new(service.name, service.id, fe_endpoints)
        })
        .filter(|service| !service.endpoints.is_empty())
        .collect::<Vec<Service>>();

    let enums = doc_enums(docs);

    let structs: Vec<Type> = docs
        .structs
        .clone()
        .into_iter()
        .map(|struct_element| struct_element.inner)
        .collect();

    serde_json::to_writer_pretty(
        &mut docs_file,
        &json!({
            "services": services,
            "enums": enums,
            "structs": structs,
        }),
    )?;
    Ok(())
}

fn error_code_enum(codes: &[ErrorCodeSchema]) -> Type {
    Type::enum_(
        "ErrorCode",
        codes
            .iter()
            .map(|code| {
                endpoint_libs::model::EnumVariant::new_with_description(
                    code.name.to_case(Case::Pascal),
                    code.description.clone(),
                    code.code,
                )
            })
            .collect(),
    )
}

fn doc_enums(data: &Data) -> Vec<Type> {
    let mut enums: Vec<Type> = data
        .enums
        .clone()
        .into_iter()
        .map(|enum_element| enum_element.inner)
        .collect();
    enums.push(error_code_enum(&data.error_codes));
    enums
}

/// Wraps ` ` around the given string
fn wrap_code_md(value: String) -> String {
    format!(r#"`{value}`"#)
}

fn format_type(field_name: &str, ty: &Type, datamodels: bool) -> String {
    match ty {
        Type::Struct { name, fields } => {
            if !datamodels {
                format!(
                    r#"{}: {}{:#}"#,
                    field_name.to_case(Case::Camel),
                    name.to_case(Case::Pascal),
                    format!(
                        "{{ {} }}",
                        fields
                            .iter()
                            .map(|x| format!("{}: {}", x.name, x.ty.to_rust_ref(false)))
                            .join(", ")
                    )
                )
            } else {
                format!(
                    r#"{}{:#}"#,
                    name.to_case(Case::Pascal),
                    format!(
                        "{{ {} }}",
                        fields
                            .iter()
                            .map(|x| format!("{}: {}", x.name, x.ty.to_rust_ref(false)))
                            .join(", ")
                    )
                )
            }
        }
        Type::StructTable { struct_ref } => {
            format!(
                "{}: Vec<{}>",
                field_name.to_case(Case::Camel),
                struct_ref.to_case(Case::Pascal),
            )
        }
        Type::Enum { name, variants } => {
            format!(
                "{} {{ {} }}",
                name.to_case(Case::Pascal),
                variants.iter().map(|v| &v.name).join(", ")
            )
        }
        Type::EnumRef { name, prefixed_name } => {
            format!(
                "{}: {}",
                field_name.to_case(Case::Camel),
                prefixed_name
                    .then(|| format!("Enum{}", name.to_case(Case::Pascal)))
                    .unwrap_or(name.to_case(Case::Pascal))
            )
        }
        // Type::DataTable { name, fields } => {
        //     format!(
        //         "{}: Vec<{}{:#}>",
        //         field_name.to_case(Case::Camel),
        //         name.to_case(Case::Pascal),
        //         format!(
        //             "{{ {} }}",
        //             fields
        //                 .iter()
        //                 .map(|x| format!(
        //                     "{}: {}",
        //                     x.name.to_case(Case::Camel),
        //                     x.ty.to_rust_ref(false)
        //                 ))
        //                 .join(", ")
        //         )
        //     )
        // }
        _ => format!("{}: {}", field_name.to_case(Case::Camel), ty.to_rust_ref(false)),
    }
}

fn format_errors(errors: &[endpoint_libs::model::EndpointErrorSchema]) -> String {
    errors
        .iter()
        .map(|error| {
            let fields = if error.fields.is_empty() {
                String::new()
            } else {
                format!(
                    " {{{}}}",
                    error
                        .fields
                        .iter()
                        .map(|field| format!("{}: {}", field.name.to_case(Case::Camel), field.ty.to_rust_ref(false)))
                        .join(", ")
                )
            };
            format!("{}({}){}", error.name, error.code, fields)
        })
        .join(", ")
}

pub fn gen_md_docs(data: &Data) -> eyre::Result<()> {
    let docs_filename = data.project_root.join("docs").join("README.md");
    let mut docs_file = File::create(docs_filename)?;
    writeln!(
        &mut docs_file,
        r#"
# API Reference

## Structs/Datamodels

```rust
{}
```
---

## Enums

```rust
{}
```
---

        "#,
        data.structs
            .iter()
            .map(|s| format!(
                "struct {:#}\n",
                format_type(&s.inner.to_rust_ref(false), &s.inner, true)
            ))
            .join("\n\n"),
        data.enums
            .iter()
            .map(|e| e.inner.clone())
            .chain(std::iter::once(error_code_enum(&data.error_codes)))
            .map(|e| format!("enum {:#}\n", format_type(&e.to_rust_ref(false), &e, true)))
            .join("\n\n")
    )?;
    for s in &data.services {
        writeln!(
            &mut docs_file,
            r#"
## {} Server
ID: {}
### Endpoints
|Code|Name|Parameters|Response|Description|FE Facing|Errors|
|-----------|-----------|----------|--------|-----------|-----------|-----------|"#,
            s.name, s.id
        )?;
        for e in &s.endpoints {
            writeln!(
                &mut docs_file,
                "|{}|{}|{}|{}|{}|{}|{}|",
                e.schema.code,
                e.schema.name,
                e.schema
                    .parameters
                    .iter()
                    .map(|x| wrap_code_md(format_type(&x.name, &x.ty, false)))
                    .join(", "),
                e.schema
                    .returns
                    .iter()
                    .map(|x| wrap_code_md(format_type(&x.name, &x.ty, false)))
                    .join(", "),
                e.schema.description,
                e.frontend_facing,
                format_errors(&e.schema.errors),
            )?;
        }
    }
    Ok(())
}

/// Writes `docs/<service>_mcp_tools.json` for each service: the MCP tool list
/// (name, description, inputSchema, outputSchema) exactly as a server built
/// from these schemas will report it via `tools/list`. Intended for review —
/// schema changes show up as diffs in these files.
pub fn gen_mcp_tools_json(data: &Data) -> eyre::Result<()> {
    let docs_dir = data.project_root.join("docs");
    create_dir_all(&docs_dir)?;

    let mut registry = endpoint_libs::model::TypeRegistry::new();
    registry.add_all(crate::rust::shared_type_definitions(data).iter());
    for service in &data.services {
        for endpoint in &service.endpoints {
            registry.add_endpoint(&endpoint.schema);
        }
    }

    for service in &data.services {
        let tools = service
            .endpoints
            .iter()
            .map(|endpoint| {
                let schema = &endpoint.schema;
                let mut tool = json!({
                    "name": schema.tool_name(),
                    "code": schema.code,
                    "description": schema.description,
                    "frontendFacing": endpoint.frontend_facing,
                    "inputSchema": schema.to_mcp_input_schema(&registry).with_context(|| {
                        format!("endpoint {} ({})", schema.name, schema.code)
                    })?,
                });
                if !schema.returns.is_empty() {
                    tool["outputSchema"] = schema
                        .to_mcp_output_schema(&registry)
                        .with_context(|| format!("endpoint {} ({})", schema.name, schema.code))?;
                }
                if schema.stream_response.is_some() {
                    tool["streaming"] = json!(true);
                }
                Ok(tool)
            })
            .collect::<eyre::Result<Vec<_>>>()?;

        let filename = docs_dir.join(format!("{}_mcp_tools.json", service.name));
        let file = File::create(&filename)
            .with_context(|| format!("Failed to create MCP tools file: {}", filename.display()))?;
        serde_json::to_writer_pretty(file, &json!({ "tools": tools }))?;
    }
    Ok(())
}

pub fn gen_systemd_services(data: &Data, app_name: &str, user: &str) -> eyre::Result<()> {
    create_dir_all(data.project_root.join("etc").join("systemd"))?;

    for srv in &data.services {
        let service_filename = data
            .project_root
            .join("etc")
            .join("systemd")
            .join(format!("{}_{}.service", app_name, srv.name));
        let mut service_file = File::create(&service_filename)?;
        let v = get_systemd_service(app_name, &srv.name, user);
        write!(&mut service_file, "{v}")?;
    }
    Ok(())
}

pub fn gen_error_message_md(root: &Path, codes: &[ErrorCodeSchema]) -> eyre::Result<()> {
    let doc_filename = root.join("docs").join("error_codes").join("error_codes.md");

    if let Some(parent) = doc_filename.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut doc_file = File::create(doc_filename)?;
    writeln!(
        &mut doc_file,
        r#"
# Error Messages
|Error Code|Error Name|Description|
|----------|----------|-----------|"#,
    )?;
    for item in codes {
        writeln!(&mut doc_file, "|{}|{}|{}|", item.code, item.name, item.description)?;
    }
    Ok(())
}

/// Writes `docs/openapi-README.md`: what the two specification documents are,
/// the synthetic-path warning, and how to consume them.
///
/// Generated rather than hand-written so it cannot drift out of sync with the
/// emitters, and so every consumer repo gets it without copying a file around.
pub fn gen_spec_readme(project_root: &Path, openapi: bool, asyncapi: bool) -> eyre::Result<()> {
    let docs_dir = project_root.join("docs");
    create_dir_all(&docs_dir)?;
    let filename = docs_dir.join("openapi-README.md");
    let mut file =
        File::create(&filename).with_context(|| format!("Failed to create spec README: {}", filename.display()))?;

    // The table lists only what was actually emitted — the specs are opt-in, so
    // a README promising an openapi.json that does not exist would be a lie.
    write!(&mut file, "{SPEC_README_HEAD}")?;
    if asyncapi {
        writeln!(
            &mut file,
            "| `asyncapi.json` | AsyncAPI 3.0. **The authoritative description of the wire protocol.** |"
        )?;
    }
    if openapi {
        writeln!(
            &mut file,
            "| `openapi.json` | OpenAPI 3.1. A projection for HTTP tooling. Not servable — see below. |"
        )?;
    }
    writeln!(
        &mut file,
        "| `<service>_mcp_tools.json` | The MCP tool list a server reports via `tools/list`. |"
    )?;
    writeln!(
        &mut file,
        "| `services.json`, `README.md` | Human-facing dumps of the same model. |"
    )?;

    if openapi {
        write!(&mut file, "{SPEC_README_OPENAPI}")?;
    }
    write!(&mut file, "{SPEC_README_TAIL}")?;
    Ok(())
}

const SPEC_README_HEAD: &str = r#"# API specification documents

Generated by `endpoint-gen` from the RON endpoint definitions in `config/`.
**Do not edit these by hand** — regenerate with `endpoint-gen`, and prove they
are current with `endpoint-gen --check` (which exits non-zero on drift).

The specification documents are opt-in: pass `--openapi` and/or `--asyncapi`.
Upgrading the generator does not add them on its own.

| File | What it is |
|---|---|
"#;

const SPEC_README_OPENAPI: &str = r#"
## The OpenAPI document does not describe a servable HTTP API

There are no URLs in this system. The transport is a persistent WebSocket
carrying `{method, seq, params}` frames, where `method` is an endpoint code, plus
MCP JSON-RPC 2.0 on the same socket.

OpenAPI needs paths, so they are synthesized:

```
POST /{serviceName}/{endpoint_snake_name}     e.g. POST /adminApi/delete_app
  operationId: {serviceName}_{endpoint_snake_name}
  tags:        [{serviceName}]
```

The service segment is not decoration. Endpoint names are only conventionally
unique — the RON namespace is `(service_name, service_id)` — so a flat
`/rpc/{Name}` scheme would collide the moment two services reuse a name.

**Generating an HTTP client from `openapi.json` and pointing it at the server
will not work.** Use `asyncapi.json` to implement a real client.


## Consuming these

**Third-party SDKs.** Run `openapi-generator` against a `--public-only`
document. Expect the synthetic paths to be wrong for real use — validate that it
*generates*, not that it *connects*.

```bash
endpoint-gen --public-only
```

**OpenAPI → MCP bridging.** Point a bridge such as `rmcp-openapi` at
`openapi.json` and compare its tool list against the `*_mcp_tools.json` files.
A mismatch means the hand-rolled MCP metadata and the emitted spec disagree, and
one of them is lying to an agent.

**Spec-driven fuzzing.** Schemathesis wants a real HTTP surface, which this
system does not have. It stays future work behind a REST adapter that does not
exist yet.
"#;

const SPEC_README_TAIL: &str = r#"
## Vendor extensions

| Extension | Meaning |
|---|---|
| `x-endpoint-code` | The wire `method` code. |
| `x-roles` | RBAC roles required. Deliberately *not* OAuth2 scopes — encoding them as scopes makes generators emit auth flows that do not exist. |
| `x-frontend-facing` | Whether the endpoint is public. Drives `--public-only`. |
| `x-stream-response` | The server may send multiple responses for one request. |
| `x-error-codes` | Error codes the operation may return, resolved against the global catalog. |
| `x-method-map` | AsyncAPI only: endpoint code → request message name. |
| `x-framing` | AsyncAPI only: byte layout of the non-WebSocket local transport. |

## Errors

The protocol has no status codes, so no per-error HTTP statuses are invented.
Every operation has a `default` response over the shared `ErrorEnvelope` schema
(`code`, `message`, `params`), and `x-error-codes` lists what that operation can
actually return.
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::definitions::{EndpointSchemaElement, RustGenConfig};
    use endpoint_libs::model::Field;

    #[test]
    fn mcp_tools_json_is_written_per_service() {
        let dir = std::env::temp_dir().join(format!("endpointgen-mcp-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        let data = Data {
            project_name: "test".into(),
            project_root: dir.clone(),
            output_dir: dir.clone(),
            services: vec![GenService::new(
                "user".to_string(),
                1,
                vec![EndpointSchemaElement {
                    frontend_facing: true,
                    config: RustGenConfig::default(),
                    schema: EndpointSchema::new(
                        "UserGetProfile",
                        10010,
                        vec![Field::new("user_id", Type::Int64)],
                        vec![Field::new("ok", Type::Boolean)],
                    )
                    .with_description("Fetches a user profile."),
                }],
            )],
            enums: vec![],
            structs: vec![],
            error_codes: vec![],
        };

        gen_mcp_tools_json(&data).unwrap();

        let out = std::fs::read_to_string(dir.join("docs").join("user_mcp_tools.json")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
        let tool = &parsed["tools"][0];
        assert_eq!(tool["name"], json!("user_get_profile"));
        assert_eq!(tool["code"], json!(10010));
        assert_eq!(tool["inputSchema"]["required"], json!(["userId"]));
        assert_eq!(tool["outputSchema"]["properties"]["ok"]["type"], json!("boolean"));

        std::fs::remove_dir_all(&dir).ok();
    }
}
