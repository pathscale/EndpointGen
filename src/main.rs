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
use std::env;
use std::result::Result::Ok;
use walkdir::WalkDir;
use crate::type_registry::UserTypeRegistry;
use regex::{Captures, Regex};

pub mod docs;
pub mod rust;
pub mod service;
pub mod sql;
pub mod type_registry;

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

    /// Identifier file for the Datatable
    #[arg(short, long)]
    identifier_dir: Option<String>,
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


pub fn replace_latency_with_datatable(input: &str, replacement_array: &[String]) -> String {
    let re = Regex::new(r#"ty:\s*"([^"]+)",\n"#).expect("Invalid regex pattern");

    let mut replacement_index = 0;

    let result = re.replace_all(input, |_: &Captures| {
        if replacement_index < replacement_array.len() {
            let replacement = replacement_array[replacement_index].clone();
            replacement_index += 1; 
            replacement
        } else {
            String::from("") 
        }
    });

    result.to_string()
}

fn process_file(file_path: &Path, identifier_path: &Path) -> Result<Definition> {
    // Ensure the identifier file has the correct extension
    // println!("Identifier path: {:?}", identifier_path);
    match identifier_path.extension().and_then(|ext| ext.to_str()) {
        Some("conf") => (),
        _ => return Err(eyre!("Identifier file must have a .conf extension")),
    }

    match file_path.extension() {
        Some(extension) if extension == "ron" => {
            // Read the RON file content
            let file_string = fs::read_to_string(file_path)?;

            // Check if the identifier .conf file exists
            if !identifier_path.exists() {
                println!("The RON file does not exist at: {:?}", identifier_path);
                return Err(eyre!("Identifier .conf file not found at {:?}", identifier_path));
            }

            // Load the replacement array from the .conf file
            let replacement_array =
                UserTypeRegistry::from_ron_file(identifier_path.to_str().unwrap());

            // Replace content in the RON file using the replacement array
            let replaced_string = replace_latency_with_datatable(&file_string, &replacement_array);

            println!("\nThe modified RON file content:\n{:?}", &replaced_string);

            // Parse the modified content into a Config object
            let config_file: Config = from_str(&replaced_string)?;

            // Return the extracted Definition from the Config
            Ok(config_file.definition)
        }
        _ => Err(eyre!(
            "Non-RON file or file without extension in config directory"
        )),
    }
}


fn process_input_files(dir: PathBuf, identifier_dir: PathBuf) -> Result<Vec<Definition>> {
    // Collect files from both directories
    let dir_files = collect_files_from_dir(&dir)?;
    let identifier_files = collect_files_from_dir(&identifier_dir)?;

    let mut rust_configs: Vec<Definition> = vec![];

    // Process each combination of files from both directories
    for dir_file in dir_files {
        for identifier_file in &identifier_files {
            match process_file(&dir_file, identifier_file) {
                Ok(rust_config) => rust_configs.push(rust_config),
                Err(err) => eprintln!(
                    "Error processing dir file: {dir_file:?} with identifier file: {identifier_file:?}, Error: {err}"
                ),
            }
        }
    }

    Ok(rust_configs)
}
// Helper function to collect all files from a given directory and its subdirectories
fn collect_files_from_dir(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut paths: Vec<PathBuf> = WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok()) // Filter out errors during traversal
        .filter(|e| e.file_type().is_file()) // Only collect files (not directories)
        .map(|e| e.into_path()) // Convert DirEntry to PathBuf
        .collect();

    paths.sort();
    Ok(paths)
}

struct InputObjects {
    services: Vec<Service>,
    enums: Vec<Type>,
}

fn build_object_lists(dir: PathBuf, identifier_dir: PathBuf) -> eyre::Result<InputObjects> {
    let rust_configs = process_input_files(dir, identifier_dir)?;

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

    let identifier_dir = {
        if let Some(identifier_dir) = &args.identifier_dir {
            PathBuf::from_str(&identifier_dir)?
        } else {
            env::current_dir()?
        }
    };

    let version_config = read_version_file(&config_dir.join("version.toml"))
        .wrap_err("Error opening version.toml. Make sure it exists and is structured correctly")?;

    check_compatibility(version_config)?;

    let output_dir = generation_root.join("generated");

    let input_objects = build_object_lists(config_dir, identifier_dir)?;

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
