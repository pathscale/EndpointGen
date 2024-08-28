use std::{env, path::PathBuf};

use endpoint_gen::Data;

mod gen_src;

fn main() -> eyre::Result<()> {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../docs/error_codes/error_codes.json");

    // Set these up to match your environment
    let current_dir = env::current_dir()?;
    let root = current_dir.parent().unwrap(); // This should evaluate to the root of your project where the project Cargo.toml can be found
    let output_dir = &current_dir.join("generated"); // This should evaluate to the `<root>/gen/generated/` dir

    let data = Data {
        project_root: PathBuf::from(&root),
        output_dir: PathBuf::from(output_dir),
        services: gen_src::services::get_services(),
        enums: gen_src::enums::get_enums(),
        pg_funcs: gen_src::proc_funcs::get_proc_functions(),
    };

    endpoint_gen::main(data)?;

    Ok(())
}
