use std::fs::{create_dir_all, File, OpenOptions};
use std::io::Write;
use std::path::Path;

use endpoint_libs::model::{Field, Service, Type};
use eyre::Context;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::rust::ToRust;
use crate::service::get_systemd_service;
use crate::Data;

#[derive(Debug, Serialize, Deserialize)]
struct Docs {
    services: Vec<Service>,
    enums: Vec<Type>,
}

pub fn gen_services_docs(docs: &Data) -> eyre::Result<()> {
    let docs_filename = docs.project_root.join("docs").join("services.json");

    // Ensure the parent directories exist
    if let Some(parent) = docs_filename.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut docs_file = File::create(&docs_filename)
        .with_context(|| format!("Failed to create docs file: {}", docs_filename.display()))?;
    serde_json::to_writer_pretty(
        &mut docs_file,
        &json!({
            "services": docs.services,
            "enums": docs.enums,
        }),
    )?;
    Ok(())
}

pub fn gen_md_docs(data: &Data) -> eyre::Result<()> {
    let docs_filename = data.project_root.join("docs").join("README.md");
    let mut docs_file = File::create(docs_filename)?;
    for s in &data.services {
        writeln!(
            &mut docs_file,
            r#"
# {name} Server ({id})
## Endpoints"#,
            name = s.name,
            id = s.id
        )?;

        for e in &s.endpoints {
            writeln!(
                &mut docs_file,
                r#"
### {name} ({code})

{description}"#,
                name = e.name,
                code = e.code,
                description = e.description
            )?;

            gen_fields_table(&mut docs_file, "Parameters", &e.parameters)?;
            gen_fields_table(&mut docs_file, "Returns", &e.returns)?;
        }
    }
    Ok(())
}

fn gen_fields_table(docs_file: &mut File, header: &str, fields: &[Field]) -> eyre::Result<()> {
    if fields.is_empty() {
        return Ok(());
    }

    writeln!(
        docs_file,
        r#"
#### {header}

|Name|Type|Description|
|----|----|-----------|"#
    )?;
    for field in fields {
        writeln!(
                    docs_file,
                    "|{name}|`{type}``|{description}|",
                    name = field.name, description = field.description,
                    type = field.ty.to_rust_ref(true)                )?;
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
