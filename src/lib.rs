pub mod docs;
pub mod model;
pub mod rust;
pub mod service;
pub mod sql;

use eyre::*;

use crate::model::{ProceduralFunction, Service, Type};
use std::env;
use std::fs::create_dir_all;
use std::path::PathBuf;

pub struct Data {
    pub project_root: PathBuf,
    pub output_dir: PathBuf,
    pub services: Vec<Service>,
    pub enums: Vec<Type>,
    pub pg_funcs: Vec<ProceduralFunction>,
}
pub fn main(data: Data) -> Result<()> {
    let mut root = env::current_dir()?;
    loop {
        if root.join(".cargo").exists() {
            break;
        }
        root = root.parent().unwrap().to_owned();
    }
    let root = root.to_str().unwrap();
    let dir = format!("{}/build", root);
    create_dir_all(&dir)?;

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
