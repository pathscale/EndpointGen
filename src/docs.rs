use crate::model::{Service, Type};
use crate::service::get_systemd_service;
use crate::Data;
use eyre::Context;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs::{create_dir_all, File};
use std::io::Write;
use std::path::Path;

#[derive(Debug, Serialize, Deserialize)]
struct Docs {
    services: Vec<Service>,
    enums: Vec<Type>,
}

pub fn gen_services_docs(docs: &Data) -> eyre::Result<()> {
    let docs_filename = docs.project_root.join("docs/services.json");
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
    let docs_filename = data.project_root.join("docs/README.md");
    let mut docs_file = File::create(docs_filename)?;
    for s in &data.services {
        writeln!(
            &mut docs_file,
            r#"
# {} Server
ID: {}
## Endpoints
|Method Code|Method Name|Parameters|Response|Description|
|-----------|-----------|----------|--------|-----------|"#,
            s.name, s.id
        )?;
        for e in &s.endpoints {
            writeln!(
                &mut docs_file,
                "|{}|{}|{}|{}|{}|",
                e.code,
                e.name,
                e.parameters.iter().map(|x| x.name.to_string()).join(", "),
                e.returns.iter().map(|x| x.name.to_string()).join(", "),
                e.description
            )?;
        }
    }
    Ok(())
}

pub fn gen_systemd_services(data: &Data, app_name: &str, user: &str) -> eyre::Result<()> {
    create_dir_all(data.project_root.join("etc/systemd"))?;

    for srv in &data.services {
        let service_filename = data
            .project_root
            .join(format!("etc/systemd/{}_{}.service", app_name, srv.name));
        let mut service_file = File::create(&service_filename)?;
        let v = get_systemd_service(app_name, &srv.name, user);
        write!(&mut service_file, "{}", v)?;
    }
    Ok(())
}

pub fn get_error_messages(root: &Path) -> eyre::Result<ErrorMessages> {
    let def_filename = root.join("docs/error_codes/error_codes.json");
    let def_file = std::fs::read(def_filename)?;
    let definitions: ErrorMessages = serde_json::from_slice(&def_file)?;
    Ok(definitions)
}

pub fn gen_error_message_md(root: &Path) -> eyre::Result<()> {
    let definitions = get_error_messages(root)?;
    let doc_filename = root.join("docs/error_codes/error_codes.md");
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
