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
