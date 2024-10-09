use std::{fs, path::PathBuf, str::FromStr};

use endpoint_libs::model::{ProceduralFunction, Service, Type};
use eyre::*;

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
    /// The input TOML file for services
    #[arg(short, long, required = true)]
    service_file: String,

    /// The input TOML file for enums
    #[arg(short, long, required = true)]
    enum_file: String,

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
    pub enums: Vec<Type>,
}

fn main(data: Data) -> Result<()> {
    let args = Cli::parse();

    let project_root: PathBuf = {
        if let Some(output_dir) = &args.output {
            PathBuf::from_str(output_dir)
        } else {
            env::current_dir()?
        }
    };

    let output_dir = project_root.join("generated");

    let service_content = fs::read_to_string(&args.service_file)?;
    let services: ServicesConfig = toml::from_str(&service_content)?;

    let enums_content = fs::read_to_string(&args.enum_file)?;
    let enums: EnumsConfig = toml::from_str(&service_content)?;

    let data = Data {
        project_root,
        output_dir,
        services: services.services,
        enums: enums.enums,
    };

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
