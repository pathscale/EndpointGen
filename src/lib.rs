use std::path::PathBuf;

use endpoint_libs::model::{ProceduralFunction, Service, Type};
use eyre::*;

pub mod docs;
pub mod rust;
pub mod service;
pub mod sql;

pub struct Data {
    pub project_root: PathBuf,
    pub output_dir: PathBuf,
    pub services: Vec<Service>,
    pub enums: Vec<Type>,
    pub pg_funcs: Vec<ProceduralFunction>,
}
pub fn main(data: Data) -> Result<()> {
    docs::gen_services_docs(&data)?;
    docs::gen_md_docs(&data)?;
    rust::gen_model_rs(&data)?;
    sql::gen_model_sql(&data)?;
    sql::gen_db_sql(&data)?;
    rust::gen_db_rs(&data)?;
    // docs::gen_systemd_services(&data, "trading", "trading")?;
    docs::gen_error_message_md(&data.project_root)?;
    Ok(())
}
