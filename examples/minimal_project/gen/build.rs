use std::{env, path::PathBuf};

use endpoint_gen::Data;

fn main() -> eyre::Result<()> {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../docs/error_codes/error_codes.json");

    // Set these up to match your environment
    let root = env::current_dir()?; // This should evaluate to the root of your project where the project Cargo.toml can be found
    let output_dir = env::current_dir()?; // This should evaluate to the `<root>/gen/` dir

    let data = Data {
        project_root: PathBuf::from(&root),
        output_dir: PathBuf::from(root),
        services: services::get_services(),
        enums: enums::get_enums(),
        pg_funcs: proc_funcs::get_proc_functions(),
    };

    endpoint_gen::main(data)?;

    Ok(())
}
