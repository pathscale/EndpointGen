use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let cargo_toml_path = PathBuf::from(manifest_dir).join("Cargo.toml");

    // Read Cargo.toml and parse it
    let content = fs::read_to_string(&cargo_toml_path)
        .expect("Failed to read Cargo.toml");

    let toml_value: toml::Value = toml::from_str(&content)
        .expect("Failed to parse Cargo.toml");

    // Extract the endpoint-libs version from dependencies
    let libs_version = extract_endpoint_libs_version(&toml_value)
        .expect("Failed to find endpoint-libs in Cargo.toml dependencies");

    // Convert to semver compatible format using caret notation (^X.Y.Z)
    let libs_requirement = format!("^{}", libs_version);

    // Set as compile-time environment variable
    println!("cargo:rustc-env=ENDPOINT_LIBS_REQUIREMENT={}", libs_requirement);
}

fn extract_endpoint_libs_version(toml: &toml::Value) -> Option<String> {
    let deps = toml.get("dependencies")?;
    let endpoint_libs = deps.get("endpoint-libs")?;

    // Case 1: Simple version string like "1.3" or "1.3.0"
    if let Some(version_str) = endpoint_libs.as_str() {
        return Some(version_str.to_string());
    }

    // Case 2: Complex dependency table (e.g., { git = "...", version = "..." })
    if let Some(table) = endpoint_libs.as_table() {
        // Try to get explicit version field first
        if let Some(version) = table.get("version").and_then(|v| v.as_str()) {
            return Some(version.to_string());
        }

        // Case 3: Git dependency without explicit version
        // TODO: In the future, we could fetch the version from the git repo's Cargo.toml
        if table.contains_key("git") {
            eprintln!("Warning: endpoint-libs is a git dependency without explicit version.");
            eprintln!("Using placeholder version '0.0.0'. Update build.rs to fetch from git.");
            return Some("0.0.0".to_string());
        }
    }

    None
}
