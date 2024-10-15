use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    str::FromStr,
};

use clap::Parser;
use endpoint_libs::model::{EndpointSchema, Field, Service, Type};
use eyre::*;
use ron::{de::from_reader, extensions::Extensions, from_str, ser::PrettyConfig};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs::File;
use std::result::Result::Ok;
use walkdir::WalkDir;

pub mod docs;
pub mod rust;
pub mod service;
pub mod sql;

/// A simple program to process service definitions from multiple TOML files
#[derive(Parser, Debug)]
#[command(
    name = "endpoint-gen",
    about = "Generate endpoint documentation and code."
)]
struct Cli {
    /// Config directory. Will be set to current directory if not specified
    #[arg(short, long)]
    config_dir: Option<String>,

    /// Output directory for the generated files
    #[arg(short, long)]
    output_dir: Option<String>,
}

pub struct Data {
    pub project_root: PathBuf,
    pub output_dir: PathBuf,
    pub services: Vec<Service>,
    pub enums: Vec<Type>,
}

#[derive(Serialize, Deserialize)]
enum Definition {
    Service(Service),
    EndpointSchema(String, u16, EndpointSchema),
    EndpointSchemaList(String, u16, Vec<EndpointSchema>),
    Enum(Type),
    EnumList(Vec<Type>),
}

#[derive(Deserialize, Serialize)]
enum SchemaType {
    Service,
    Enum,
    EnumList,
    EndpointSchema(String, u16),
    EndpointSchemaList(String, u16),
}

#[derive(Deserialize, Serialize)]
struct Schema {
    schema_type: SchemaType,
}

fn process_file(file_path: &Path) -> eyre::Result<Definition> {
    match file_path.extension() {
        Some(extension) if extension == "ron" => {
            let file_string = std::fs::read_to_string(file_path)?;
            let config_file: Config = from_str(&file_string)?;

            return Ok(config_file.definition);
        }
        _ => Err(eyre!(
            "Non RON file OR file without extension in config dir "
        )),
    }
}

fn process_input_files(dir: PathBuf) -> eyre::Result<Vec<Definition>> {
    let root = dir.as_path();
    
    // Walk through the directory and all subdirectories
    let mut paths: Vec<PathBuf> = WalkDir::new(root)
    .into_iter()
    .filter_map(|e| e.ok())  // Filter out any errors
    .filter(|e| e.file_type().is_file()) // Only get files (not directories)
    .map(|e| e.into_path())  // Convert DirEntry to PathBuf
    .collect();

    paths.sort();

    let mut rust_configs: Vec<Definition> = vec![];
    for path in paths {
        match process_file(path.as_path()) {
            Ok(rust_config) => rust_configs.push(rust_config),
            Err(err) => eprintln!("Error processing file: {path:?}, Error: {err}"),
        }
    }

    Ok(rust_configs)
}

struct InputObjects {
    services: Vec<Service>,
    enums: Vec<Type>,
}

fn build_object_lists(dir: PathBuf) -> eyre::Result<InputObjects> {
    let rust_configs = process_input_files(dir)?;

    let mut service_schema_map: HashMap<(String, u16), Vec<EndpointSchema>> = HashMap::new();

    let mut services: Vec<Service> = vec![];

    let mut enums: Vec<Type> = vec![];

    for config in rust_configs {
        match config {
            Definition::Service(service) => services.push(service),
            Definition::EndpointSchema(service_name,service_id, endpoint_schema) => service_schema_map
                .entry((service_name, service_id))
                .or_insert(vec![])
                .push(endpoint_schema),
            Definition::EndpointSchemaList(service_name, service_id, endpoint_schemas) => service_schema_map
                .entry((service_name, service_id))
                .or_insert(vec![])
                .extend(endpoint_schemas),
            Definition::Enum(enum_type) => enums.push(enum_type),
            Definition::EnumList(enum_types) => enums.extend(enum_types),
        }
    }

    if !service_schema_map.is_empty() {
        for ((service_name, service_id), endpoint_schemas) in service_schema_map {
            services.push(Service::new(
                service_name,
                service_id,
                endpoint_schemas,
            ));
        }
    }

    // Sort services by ID
    services.sort_by(|a,b| a.id.cmp(&b.id));

    // Sort the endpoints of each service by their codes
    services.iter_mut().for_each(
        |service| service.endpoints.sort_by(|a,b| a.code.cmp(&b.code))
    );

    // Sort enums by their default ordering
    enums.sort();

    Ok(InputObjects { services, enums })
}

#[derive(Deserialize, Serialize)]
struct Config {
    definition: Definition,
}

fn main() -> Result<()> {
    let args = Cli::parse();

    let generation_root: PathBuf = {
        if let Some(output_dir) = &args.output_dir {
            PathBuf::from_str(output_dir)?
        } else {
            env::current_dir()?
        }
    };

    let config_dir = {
        if let Some(config_dir) = &args.config_dir {
            PathBuf::from_str(&config_dir)?
        } else {
            env::current_dir()?
        }
    };

    let output_dir = generation_root.join("generated");

    let input_objects = build_object_lists(config_dir)?;

    let data = Data {
        project_root: generation_root,
        output_dir,
        services: input_objects.services,
        enums: input_objects.enums,
    };

    docs::gen_services_docs(&data)?;
    docs::gen_md_docs(&data)?;
    rust::gen_model_rs(&data)?;
    sql::gen_model_sql(&data)?;
    docs::gen_error_message_md(&data.project_root)?;
    Ok(())
}
