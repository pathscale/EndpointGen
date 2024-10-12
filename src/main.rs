use std::{
    collections::HashMap, path::{Path, PathBuf}, str::FromStr
};

use clap::Parser;
use endpoint_libs::model::{EndpointSchema, Service, Type};
use eyre::*;
use serde::Deserialize;
use walkdir::WalkDir;
use std::env;
use std::result::Result::Ok;

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
    output: Option<String>,
}

pub struct Data {
    pub project_root: PathBuf,
    pub output_dir: PathBuf,
    pub services: Vec<Service>,
    pub enums: Vec<Type>,
}

enum RustConfig {
    Service(Service),
    EndpointSchema(String, EndpointSchema),
    EndpointSchemaList(String, Vec<EndpointSchema>),
    Enum(Type),
    EnumList(Vec<Type>),
}

#[derive(Deserialize)]
enum SchemaType {
    Service,
    Enum,
    EnumList,
    EndpointSchema(String),
    EndpointSchemaList(String),
}

#[derive(Deserialize)]
struct RonSchema {
    schema_type: SchemaType,
}

fn process_file(file_path: &Path) -> eyre::Result<RustConfig> {
    match file_path.extension() {
        Some(extension) if extension == "ron" => {
            let file_string = ron::to_string(&file_path)?;
            let schema: RonSchema = ron::from_str(&file_string)?;

            match schema.schema_type {
                SchemaType::Service => {
                    let service: Service = ron::from_str(&file_string)?;
                    return Ok(RustConfig::Service(service));
                },
                SchemaType::Enum => {
                    let enum_type: Type = ron::from_str(&file_string)?;
                    return Ok(RustConfig::Enum(enum_type));
                },
                SchemaType::EnumList => {
                    let enums: Vec<Type> = ron::from_str(&file_string)?;
                    return Ok(RustConfig::EnumList(enums));
                },
                SchemaType::EndpointSchema(service_name) => {
                    let endpoint_schema: EndpointSchema = ron::from_str(&file_string)?;
                    return Ok(RustConfig::EndpointSchema(service_name, endpoint_schema));
                },
                SchemaType::EndpointSchemaList(service_name) => {
                    let endpoint_schemas: Vec<EndpointSchema> = ron::from_str(&file_string)?;
                    return Ok(RustConfig::EndpointSchemaList(service_name, endpoint_schemas));
                },
            }
        },
        _ => Err(eyre!("Non RON file OR file without extension in config dir ")),
    }
}

fn process_input_files(dir: PathBuf) -> eyre::Result<Vec<RustConfig>> {
    let root = dir.as_path();
    let mut rust_configs: Vec<RustConfig> = vec![];

    // Walk through the directory and all subdirectories
    for entry in WalkDir::new(root).into_iter().filter_map(Result::ok) {
        let path = entry.path();

        if path.is_file() {
            match process_file(path) {
                Ok(rust_config) => rust_configs.push(rust_config),
                Err(err) => eprintln!("Error processing file: {path:?}, Error: {err}"),
            }
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

    let mut service_schema_map: HashMap<String, Vec<EndpointSchema>> = HashMap::new();

    let mut services: Vec<Service> = vec![];

    let mut enums: Vec<Type> = vec![];

    for config in rust_configs {
        match config {
            RustConfig::Service(service) => services.push(service),
            RustConfig::EndpointSchema(service_name, endpoint_schema) => {
                service_schema_map.entry(service_name).or_insert(vec![]).push(endpoint_schema)
            },
            RustConfig::EndpointSchemaList(service_name, endpoint_schemas) => {
                service_schema_map.entry(service_name).or_insert(vec![]).extend(endpoint_schemas)
            },
            RustConfig::Enum(enum_type) => enums.push(enum_type),
            RustConfig::EnumList(enum_types) => enums.extend(enum_types),
        }
    }

    let mut service_num = 1;
    if !service_schema_map.is_empty() {
        for (service_name, endpoint_schemas) in service_schema_map {
            services.push(Service::new(service_name, service_num.clone(), endpoint_schemas));
            service_num += 1;
        }
    }

    Ok(InputObjects {
        services,
        enums
    })
}

fn main() -> Result<()> {
    let args = Cli::parse();

    let generation_root: PathBuf = {
        if let Some(output_dir) = &args.output {
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
