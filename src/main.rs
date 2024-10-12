use std::{
    fs::{self},
    path::PathBuf,
    str::FromStr,
};

use clap::Parser;
use endpoint_libs::model::{Service, Type};
use eyre::*;
use serde::Deserialize;
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

#[derive(Deserialize, Debug)]
struct ServicesConfig {
    pub services: Vec<Service>,
}

#[derive(Deserialize, Debug)]
struct EnumsConfig {
    pub types: Vec<Type>,
}

enum FileType {
    Service,
    Enum,
}

enum RustConfig {
    Service(ServicesConfig),
    Enum(EnumsConfig),
}

fn process_input_files(dir: PathBuf, filetype: FileType) -> eyre::Result<Vec<RustConfig>> {
    let path = dir.as_path();
    let mut rust_configs: Vec<RustConfig> = vec![];

    if path.is_dir() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                match path.extension() {
                    Some(extension) if extension == "ron" => match filetype {
                        FileType::Service => {
                            let service_content = fs::read_to_string(&path)?;
                            let services: ServicesConfig = ron::from_str(&service_content)?;

                            rust_configs.push(RustConfig::Service(services));
                        }
                        FileType::Enum => {
                            let enums_content = fs::read_to_string(&path)?;
                            let enums: EnumsConfig = ron::from_str(&enums_content)?;

                            rust_configs.push(RustConfig::Enum(enums));
                        }
                    },
                    _ => continue,
                }
            }
        }
    } else {
        return Err(eyre!("The provided path is not a directory."));
    }
    Ok(rust_configs)
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

    let services = {
        let mut services: Vec<Service> = vec![];

        match process_input_files(config_dir.join("services"), FileType::Service) {
            Ok(rust_configs) => {
                for config in rust_configs {
                    match config {
                        RustConfig::Service(services_config) => {
                            services.extend(services_config.services)
                        }
                        RustConfig::Enum(_) => panic!["Service input files returning enums!"],
                    }
                }
            }
            Err(err) => {
                panic!["Error processing input service files: {err}"]
            }
        }
        services
    };

    let enums = {
        let mut enums: Vec<Type> = vec![];
        match process_input_files(config_dir.join("enums"), FileType::Enum) {
            Ok(rust_configs) => for config in rust_configs {
                match config {
                    RustConfig::Service(_) => panic!["Enum input files returning services!"],
                    RustConfig::Enum(enums_config) => enums.extend(enums_config.types),
                }
            },
            Err(err) => {
                panic!["Error processing input enum files: {err}"]
            }
        }
        enums
    };

    let data = Data {
        project_root: generation_root,
        output_dir,
        services,
        enums,
    };

    docs::gen_services_docs(&data)?;
    docs::gen_md_docs(&data)?;
    rust::gen_model_rs(&data)?;
    sql::gen_model_sql(&data)?;
    docs::gen_error_message_md(&data.project_root)?;
    Ok(())
}
