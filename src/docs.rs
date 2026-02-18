use crate::definitions::{EnumElement, GenService, StructElement};
use crate::rust::ToRust;
use crate::service::get_systemd_service;
use convert_case::{Case, Casing};
use endpoint_libs::model::{EndpointSchema, Service, Type};
use eyre::Context;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs::{File, OpenOptions, create_dir_all};
use std::io::Write;
use std::path::{Path, PathBuf};

pub struct Data {
    pub project_root: PathBuf,
    pub output_dir: PathBuf,
    pub services: Vec<GenService>,
    pub enums: Vec<EnumElement>,
    pub structs: Vec<StructElement>,
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

    let enums: Vec<Type> = docs
        .enums
        .clone()
        .into_iter()
        .map(|enum_element| enum_element.inner)
        .collect();

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

/// Wraps ` ` around the given string
fn wrap_code_md(value: String) -> String {
    format!(r#"`{value}`"#)
}

fn format_type(field_name: &str, ty: &Type) -> String {
    match ty {
        Type::Struct { name, fields } => {
            format!(
                r#"{}: {}{:#}"#,
                field_name.to_case(Case::Camel),
                name.to_case(Case::Camel),
                format!(
                    "{{ {} }}",
                    fields
                        .iter()
                        .map(|x| format!("{}: {}", x.name.to_string(), x.ty.to_rust_ref(false)))
                        .join(", ")
                )
            )
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
        Type::EnumRef {
            name,
            prefixed_name,
        } => {
            format!(
                "{}: {}",
                field_name.to_case(Case::Camel),
                prefixed_name
                    .then(|| format!("Enum{}", name.to_case(Case::Pascal)))
                    .unwrap_or(name.to_case(Case::Pascal))
            )
        }
        Type::DataTable { name, fields } => {
            format!(
                "{}: Vec<{}{:#}>",
                field_name.to_case(Case::Camel),
                name.to_case(Case::Pascal),
                format!(
                    "{{ {} }}",
                    fields
                        .iter()
                        .map(|x| format!(
                            "{}: {}",
                            x.name.to_case(Case::Camel),
                            x.ty.to_rust_ref(false)
                        ))
                        .join(", ")
                )
            )
        }
        _ => format!(
            "{}: {}",
            field_name.to_case(Case::Camel),
            ty.to_rust_ref(false)
        ),
    }
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
                format_type(&s.inner.to_rust_ref(false), &s.inner)
            ))
            .join("\n\n"),
        data.enums
            .iter()
            .map(|e| format!(
                "enum {:#}\n",
                format_type(&e.inner.to_rust_ref(false), &e.inner)
            ))
            .join("\n\n")
    )?;
    for s in &data.services {
        writeln!(
            &mut docs_file,
            r#"
## {} Server
ID: {}
### Endpoints
|Code|Name|Parameters|Response|Description|FE Facing|
|-----------|-----------|----------|--------|-----------|-----------|"#,
            s.name, s.id
        )?;
        for e in &s.endpoints {
            writeln!(
                &mut docs_file,
                "|{}|{}|{}|{}|{}|{}|",
                e.schema.code,
                e.schema.name,
                e.schema
                    .parameters
                    .iter()
                    .map(|x| wrap_code_md(format_type(&x.name, &x.ty)))
                    .join(", "),
                e.schema
                    .returns
                    .iter()
                    .map(|x| wrap_code_md(format_type(&x.name, &x.ty)))
                    .join(", "),
                e.schema.description,
                e.frontend_facing,
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

pub fn get_error_messages(root: &Path) -> eyre::Result<ErrorMessages> {
    let def_filename = root
        .join("docs")
        .join("error_codes")
        .join("error_codes.json");

    // Ensure the parent directories exist, and create the file
    if let Some(parent) = def_filename.parent() {
        std::fs::create_dir_all(parent)?;
    }

    if !def_filename.exists() {
        let _file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&def_filename)?;
    }

    // Read the file contents
    let def_file = std::fs::read(&def_filename)?;

    if def_file.is_empty() {
        Ok(ErrorMessages {
            language: String::from("TODO"),
            codes: vec![ErrorMessage {
                code: 0,
                symbol: String::from("XXX"),
                message: String::from("Please populate error_codes.json"),
                source: String::from("None"),
            }],
        })
    } else {
        let definitions: ErrorMessages = serde_json::from_slice(&def_file)?;
        Ok(definitions)
    }
}

pub fn gen_error_message_md(root: &Path) -> eyre::Result<()> {
    let definitions = get_error_messages(root)?;
    let doc_filename = root.join("docs").join("error_codes").join("error_codes.md");
    let mut doc_file = File::create(doc_filename)?;
    writeln!(
        &mut doc_file,
        r#"
# Error Messages
|Error Code|Error Symbol|Error Message|Error Source|
|----------|------------|-------------|------------|"#,
    )?;
    for item in definitions.codes {
        writeln!(
            &mut doc_file,
            "|{}|{}|{}|{}|",
            item.code, item.symbol, item.message, item.source
        )?;
    }
    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorMessages {
    pub language: String,
    pub codes: Vec<ErrorMessage>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorMessage {
    pub code: i64,
    #[serde(default)]
    pub symbol: String,
    pub message: String,
    #[serde(default)]
    pub source: String,
}
