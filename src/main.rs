use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    str::FromStr,
};

use clap::Parser;
use endpoint_libs::model::{EndpointSchema, Service, Type};
use eyre::*;
use ron::from_str;
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use tracing_appender::rolling::Rotation;
use std::env;
use std::result::Result::Ok;
use walkdir::WalkDir;
use regex::Regex;
use regex::Captures;
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

#[derive(Debug, Deserialize, Clone)]
struct Identifier {
    name: String,
    identifier: String,
}

#[derive(Debug, Deserialize)]
struct IdentifierConfig {
    definition: Vec<Identifier>,
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

fn process_file(file_path: &Path, identifiers: &[Identifier]) -> eyre::Result<Definition> {
    let file_string = std::fs::read_to_string(file_path)
        .wrap_err_with(|| format!("Failed to read file: {:?}", file_path))?;

    if file_string
        .lines()
        .any(|line| line.trim_start().starts_with("IdentifierConfig"))
    {
        return Err(eyre!("File contains IdentifierConfig, skipping processing."));
    }

    if file_path.extension().and_then(|ext| ext.to_str()) != Some("ron") {
        return Err(eyre!("Non-RON file or file without correct extension."));
    }

    let replaced_string = replace_latency_with_identifiers(&file_string, identifiers);

    let config_file: Config = from_str(&replaced_string)
        .wrap_err_with(|| format!("Failed to parse RON content from file: {:?}", file_path))?;

    Ok(config_file.definition)
}

pub fn replace_latency_with_identifiers(input: &str, identifiers: &[Identifier]) -> String {
    let identifier_map: HashMap<&str, &str> = identifiers
        .iter()
        .map(|id| (id.name.as_str(), id.identifier.as_str()))
        .collect();
    
    let re = Regex::new(r#"ty:\s*"([^"]+)","#).expect("Invalid regex pattern");

    let result = re.replace_all(input, |captures: &Captures| {
        let name = captures.get(1).map_or("", |m| m.as_str());

        identifier_map.get(name).unwrap_or(&name).to_string()
    });

    result.to_string()
}


fn process_identifier_file(file_path: &Path) -> Result<Vec<Identifier>> {
    let file_content = fs::read_to_string(file_path)
        .wrap_err(format!("Failed to read file: {:?}", file_path))?;

    let identifier_config: IdentifierConfig = from_str(&file_content)
        .wrap_err(format!("Failed to parse RON from file: {:?}", file_path))?;

    Ok(identifier_config.definition)
}

fn process_identifier_files(dir: &Path) -> Result<Vec<Identifier>> {
    let mut identifiers = Vec::new();

    let paths: Vec<PathBuf> = WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.into_path())
        .collect();

    for path in paths {
        if path.extension().map(|ext| ext == "ron").unwrap_or(false) {
            let file_content = fs::read_to_string(&path)
                .wrap_err(format!("Failed to read file: {:?}", &path))?;

            if file_content
                .lines()
                .any(|line| line.trim_start().starts_with("IdentifierConfig"))
            {
                match process_identifier_file(&path) {
                    Ok(identifier_list) => identifiers.extend(identifier_list),
                    Err(err) => eprintln!("Error processing file: {path:?}, Error: {err}"),
                }
            }
        }
    }

    Ok(identifiers)
}

fn process_input_files(dir: PathBuf, identifiers: Vec<Identifier>) -> eyre::Result<Vec<Definition>> {
    let root = dir.as_path();

    // Walk through the directory and all subdirectories
    let mut paths: Vec<PathBuf> = WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok()) // Filter out any errors
        .filter(|e| e.file_type().is_file()) // Only get files (not directories)
        .map(|e| e.into_path()) // Convert DirEntry to PathBuf
        .collect();

    paths.sort();

    let mut rust_configs: Vec<Definition> = vec![];
    for path in paths {
        match process_file(path.as_path(), &identifiers) {
            Ok(rust_config) => rust_configs.push(rust_config),
            Err(err) => match path.file_name() {
                Some(name) if name.to_str().unwrap() == "version.toml" => (),
                Some(_) => eprintln!("Error processing file: {path:?}, Error: {err}"),
                None => (),
            },
        }
    }

    Ok(rust_configs)
}

struct InputObjects {
    services: Vec<Service>,
    enums: Vec<Type>,
}

fn build_object_lists(dir: PathBuf, identifiers: Vec<Identifier>) -> eyre::Result<InputObjects> {
    let rust_configs = process_input_files(dir, identifiers)?;

    let mut service_schema_map: HashMap<(String, u16), Vec<EndpointSchema>> = HashMap::new();

    let mut services: Vec<Service> = vec![];

    let mut enums: Vec<Type> = vec![];

    for config in rust_configs {
        match config {
            Definition::Service(service) => services.push(service),
            Definition::EndpointSchema(service_name, service_id, endpoint_schema) => {
                service_schema_map
                    .entry((service_name, service_id))
                    .or_insert(vec![])
                    .push(endpoint_schema)
            }
            Definition::EndpointSchemaList(service_name, service_id, endpoint_schemas) => {
                service_schema_map
                    .entry((service_name, service_id))
                    .or_insert(vec![])
                    .extend(endpoint_schemas)
            }
            Definition::Enum(enum_type) => enums.push(enum_type),
            Definition::EnumList(enum_types) => enums.extend(enum_types),
        }
    }

    if !service_schema_map.is_empty() {
        for ((service_name, service_id), endpoint_schemas) in service_schema_map {
            services.push(Service::new(service_name, service_id, endpoint_schemas));
        }
    }

    // Sort services by ID
    services.sort_by(|a, b| a.id.cmp(&b.id));

    // Sort the endpoints of each service by their codes
    services
        .iter_mut()
        .for_each(|service| service.endpoints.sort_by(|a, b| a.code.cmp(&b.code)));

    // Sort enums by their default ordering
    enums.sort();

    Ok(InputObjects { services, enums })
}

#[derive(Deserialize, Serialize)]
struct Config {
    definition: Definition,
}

#[derive(Debug, Deserialize)]
struct VersionConfig {
    binary: BinaryVersion,
    libs: LibsVersion,
}

/// The version of the binary that the config files require
#[derive(Debug, Deserialize)]
struct BinaryVersion {
    version: String, // This will use semver version constraints
}

/// The version of endpoint-libs that the caller is using
#[derive(Debug, Deserialize)]
struct LibsVersion {
    version: String, // This will use semver version constraints
}

fn read_version_file(path: &Path) -> eyre::Result<VersionConfig> {
    let content = fs::read_to_string(path)?;
    let version_config: VersionConfig = toml::from_str(&content)?;
    Ok(version_config)
}

fn check_compatibility(
    version_config: VersionConfig
) -> eyre::Result<()> {
    let current_crate_version = Version::parse(&get_crate_version()).unwrap();

    let binary_version_req = VersionReq::parse(&version_config.binary.version).unwrap();

    // The version of endpoint-libs that we require to be used with this version of endpoint-gen
    let libs_version_requirement = ">=1.0.3";

    let libs_version_req = VersionReq::parse(libs_version_requirement).unwrap();

    let caller_libs_version = Version::parse(&version_config.libs.version).unwrap();

    if !binary_version_req.matches(&current_crate_version) {
        return Err(eyre!("Binary version constraint not satisfied. Version: {} is specified in version.toml. Current binary version is: {}", 
        &version_config.binary.version, &get_crate_version()));
    } else if !libs_version_req.matches(&caller_libs_version) {
        return Err(eyre!("endpoint-libs version constraint not satisfied. Version: {} is specified in version.toml. This version of endpoint-gen requires: {}", 
        caller_libs_version, libs_version_requirement));
    } else {
        return Ok(());
    }
}

fn get_crate_version() -> &'static str {
    // Get the crate version from the Cargo.toml at compile time
    env!("CARGO_PKG_VERSION")
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

    let version_config = read_version_file(&config_dir.join("version.toml"))
        .wrap_err("Error opening version.toml. Make sure it exists and is structured correctly")?;

    check_compatibility(version_config)?;

    let output_dir = generation_root.join("generated");

    let identifiers = process_identifier_files(&config_dir)?;

    let input_objects = build_object_lists(config_dir, identifiers)?;

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
