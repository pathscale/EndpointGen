use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    str::FromStr,
};

use clap::Parser;
use endpoint_gen::{
    definitions::{Definition, EndpointSchemaElement, EnumElement, GenService, StructElement},
    docs::{self, Data},
    rust,
};
use eyre::*;
use ron::from_str;
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use std::env;
use std::result::Result::Ok;
use walkdir::WalkDir;

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
            PathBuf::from_str(config_dir)?
        } else {
            env::current_dir()?
        }
    };

    let version_config = read_version_file(&config_dir.join("version.toml"))
        .wrap_err("Error opening version.toml. Make sure it exists and is structured correctly")?;

    check_compatibility(version_config)?;

    let output_dir = generation_root.join("generated");

    let input_objects = build_object_lists(config_dir)?;

    let data = Data {
        project_root: generation_root,
        output_dir,
        services: input_objects.services,
        enums: input_objects.enums,
        structs: input_objects.structs,
    };

    docs::gen_services_docs(&data)?;
    docs::gen_md_docs(&data)?;
    rust::gen_model_rs(&data)?;
    docs::gen_error_message_md(&data.project_root)?;
    Ok(())
}

fn process_file(file_path: &Path) -> eyre::Result<Option<Definition>> {
    match file_path.extension() {
        Some(extension) if extension == "ron" => {
            let file_string = std::fs::read_to_string(file_path)?;
            let config_file: Config = from_str(&file_string)?;

            Ok(Some(config_file.definition))
        }
        _ => Ok(None), // No extension or extension != .ron, safe to ignore
    }
}

fn process_input_files(dir: PathBuf) -> eyre::Result<Vec<Definition>> {
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
    let mut valid_config_files_counter = 0u32;
    for path in paths {
        match process_file(path.as_path()) {
            Ok(rust_config) => {
                if let Some(config) = rust_config {
                    rust_configs.push(config);
                    valid_config_files_counter += 1;
                }
            }
            Err(err) => match path.file_name() {
                Some(name) if name.to_str().unwrap() == "version.toml" => (),
                Some(_) => eprintln!("Error processing file: {path:?}, Error: {err}"),
                None => (),
            },
        }
    }

    // If we haven't found any files, it's better to just return here immediately
    if valid_config_files_counter == 0 {
        bail!("No valid RON config files found in given path, aborting generation process");
    }

    Ok(rust_configs)
}

struct InputObjects {
    services: Vec<GenService>,
    enums: Vec<EnumElement>,
    structs: Vec<StructElement>,
}

fn build_object_lists(dir: PathBuf) -> eyre::Result<InputObjects> {
    let rust_configs = process_input_files(dir)?;

    let mut service_schema_map: HashMap<(String, u16), Vec<EndpointSchemaElement>> = HashMap::new();

    let mut services: Vec<GenService> = vec![];

    let mut enums: Vec<EnumElement> = vec![];
    let mut structs: Vec<StructElement> = vec![];

    for config in rust_configs {
        match config {
            Definition::EndpointSchema(schema_definition) => service_schema_map
                .entry((schema_definition.service_name, schema_definition.service_id))
                .or_default()
                .push(schema_definition.schema),
            Definition::EndpointSchemaList(schema_list_definition) => service_schema_map
                .entry((
                    schema_list_definition.service_name,
                    schema_list_definition.service_id,
                ))
                .or_default()
                .extend(schema_list_definition.endpoints),
            Definition::Enum(enum_type) => enums.push(enum_type),
            Definition::EnumList(enum_types) => enums.extend(enum_types),
            Definition::Struct(struct_element) => structs.push(struct_element),
            Definition::StructList(struct_elements) => structs.extend(struct_elements),
        }
    }

    if !service_schema_map.is_empty() {
        for ((service_name, service_id), endpoint_schemas) in service_schema_map {
            services.push(GenService::new(service_name, service_id, endpoint_schemas));
        }
    }

    // Sort services by ID
    services.sort_by(|a, b| a.id.cmp(&b.id));

    // Sort the endpoints of each service by their codes
    services.iter_mut().for_each(|service| {
        service
            .endpoints
            .sort_by(|a, b| a.schema.code.cmp(&b.schema.code))
    });

    // Sort enums and structs by their default ordering
    enums.sort();
    structs.sort();

    Ok(InputObjects {
        services,
        enums,
        structs,
    })
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

fn check_compatibility(version_config: VersionConfig) -> eyre::Result<()> {
    let current_crate_version = Version::parse(get_crate_version()).unwrap();

    let binary_version_req = VersionReq::parse(&version_config.binary.version).unwrap();

    // The version of endpoint-libs that we require to be used with this version of endpoint-gen
    let libs_version_requirement = ">=1.0.3";

    let libs_version_req = VersionReq::parse(libs_version_requirement).unwrap();

    let caller_libs_version = Version::parse(&version_config.libs.version).unwrap();

    if !binary_version_req.matches(&current_crate_version) {
        Err(eyre!("Binary version constraint not satisfied. Version: {} is specified in version.toml. Current binary version is: {}", 
        &version_config.binary.version, &get_crate_version()))
    } else if !libs_version_req.matches(&caller_libs_version) {
        Err(eyre!("endpoint-libs version constraint not satisfied. Version: {} is specified in version.toml. This version of endpoint-gen requires: {}",
        caller_libs_version, libs_version_requirement))
    } else {
        Ok(())
    }
}

fn get_crate_version() -> &'static str {
    // Get the crate version from the Cargo.toml at compile time
    env!("CARGO_PKG_VERSION")
}
