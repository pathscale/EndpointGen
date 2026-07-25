use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    str::FromStr,
};

use clap::Parser;
use convert_case::{Case, Casing};
use endpoint_gen::{
    asyncapi,
    definitions::{Definition, EndpointSchemaElement, EnumElement, ErrorCodeSchema, GenService, StructElement},
    docs::{self, Data},
    error_codes::{build_error_code_catalog, validate_endpoint_error_codes, validate_reserved_enum_names},
    openapi, rust,
};
use endpoint_libs::model::Type;
use eyre::*;
use ron::from_str;
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use std::env;
use std::result::Result::Ok;
use walkdir::WalkDir;

/// A simple program to process service definitions from multiple TOML files
#[derive(Parser, Debug)]
#[command(name = "endpoint-gen", version, about = "Generate endpoint documentation and code.")]
struct Cli {
    /// Config directory. Will be set to current directory if not specified
    #[arg(short, long)]
    config_dir: Option<String>,

    /// Output directory for the generated files
    #[arg(short, long)]
    output_dir: Option<String>,

    /// Allow endpoint and enum-variant descriptions to be missing or blank
    /// (legacy behavior). By default, generation fails on empty descriptions
    /// since they produce useless MCP tool metadata and docs.
    #[arg(long)]
    allow_empty_descriptions: bool,

    /// Emit only `frontend_facing` endpoints into the OpenAPI/AsyncAPI
    /// documents — the version you would hand to a third party.
    ///
    /// Filtering is per endpoint, not per service. Does not affect the Rust
    /// output or the MCP tool lists.
    #[arg(long)]
    public_only: bool,

    /// Verify the committed artifacts match the RON instead of writing them.
    ///
    /// Regenerates everything into a temporary directory, diffs it against the
    /// `docs/` tree, and exits non-zero if anything differs or is missing.
    /// Nothing on disk is touched. Intended for CI: a committed artifact is
    /// only trustworthy if something proves it still matches its source.
    ///
    /// Only `docs/` is compared. The `generated/` Rust output is gitignored in
    /// every consumer repo, so it is not a committed artifact and cannot
    /// meaningfully drift.
    #[arg(long)]
    check: bool,
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

    let input_objects = build_object_lists(config_dir, args.allow_empty_descriptions)?;

    let data = Data {
        project_name: generation_root
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "API".into()),
        project_root: generation_root,
        output_dir,
        services: input_objects.services,
        enums: input_objects.enums,
        structs: input_objects.structs,
        error_codes: input_objects.error_codes,
    };

    if args.check {
        return run_check(&data, args.public_only);
    }

    run_generation(&data, args.public_only)
}

/// Writes every artifact rooted at `data.project_root` / `data.output_dir`.
///
/// Kept separate from `main` so `--check` can run the identical pipeline into a
/// scratch directory. If these ever diverge, `--check` starts lying.
fn run_generation(data: &Data, public_only: bool) -> Result<()> {
    let docs_data = format_for_docs(data);

    docs::gen_services_docs(&docs_data)?;
    docs::gen_md_docs(&docs_data)?;
    // Raw data (not docs_data): MCP schemas, the OpenAPI document and the
    // AsyncAPI document all camelCase field names themselves, matching the wire
    // format regardless of the snake_case_fields config.
    docs::gen_mcp_tools_json(data)?;
    openapi::gen_openapi(data, public_only)?;
    asyncapi::gen_asyncapi(data, public_only)?;
    docs::gen_spec_readme(&data.project_root)?;
    rust::gen_model_rs(data)?;
    docs::gen_error_message_md(&data.project_root, &data.error_codes)?;
    Ok(())
}

/// Regenerates into a temp directory and diffs `docs/` against the working tree.
///
/// Returns `Err` (non-zero exit) listing every drifted or missing file. Writes
/// nothing to the project.
fn run_check(data: &Data, public_only: bool) -> Result<()> {
    let scratch = tempfile::tempdir().wrap_err("failed to create scratch directory for --check")?;

    let staged = Data {
        // Same name, different location: the point of --check is to compare
        // content, so nothing but the output path may differ.
        project_name: data.project_name.clone(),
        project_root: scratch.path().to_path_buf(),
        output_dir: scratch.path().join("generated"),
        services: data.services.clone(),
        enums: data.enums.clone(),
        structs: data.structs.clone(),
        error_codes: data.error_codes.clone(),
    };
    run_generation(&staged, public_only)?;

    let staged_docs = scratch.path().join("docs");
    let committed_docs = data.project_root.join("docs");

    let mut drift: Vec<String> = vec![];
    let mut compared = 0u32;

    let mut staged_files: Vec<PathBuf> = WalkDir::new(&staged_docs)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.into_path())
        .collect();
    staged_files.sort();

    for staged_file in staged_files {
        let rel = staged_file
            .strip_prefix(&staged_docs)
            .expect("walked path is always under staged_docs");
        let expected = fs::read(&staged_file)?;

        match fs::read(committed_docs.join(rel)) {
            Ok(actual) if actual == expected => compared += 1,
            Ok(_) => {
                compared += 1;
                drift.push(format!("  drifted:  docs/{}", rel.display()));
            }
            Err(_) => drift.push(format!("  missing:  docs/{}", rel.display())),
        }
    }

    if drift.is_empty() {
        println!("endpoint-gen --check: {compared} generated file(s) match the RON definitions.");
        return Ok(());
    }

    bail!(
        "Generated artifacts are out of date with the RON definitions:\n{}\n\n\
         Regenerate with `endpoint-gen` (no --check) and commit the result.",
        drift.join("\n")
    );
}

/// Formats fields of endpoint input/return params and fields of structs from snake to camel case if enabled
/// This allows us to still have snake case field names in our rust code, but FE facing docs can remain camel case
/// We already use serde camelCase renaming, so this should have no effect on serializing/deserializing
fn format_for_docs(data: &Data) -> Data {
    fn camel_case_field(mut field: endpoint_libs::model::Field) -> endpoint_libs::model::Field {
        field.name = field.name.to_case(Case::Camel);
        field
    }

    let formatted_services = data
        .services
        .clone()
        .into_iter()
        .map(|mut gen_service| {
            gen_service.endpoints = gen_service
                .endpoints
                .into_iter()
                .map(|mut endpoint| {
                    if endpoint.config.snake_case_fields {
                        endpoint.schema.parameters =
                            endpoint.schema.parameters.into_iter().map(camel_case_field).collect();

                        endpoint.schema.returns = endpoint.schema.returns.into_iter().map(camel_case_field).collect();

                        endpoint.schema.errors = endpoint
                            .schema
                            .errors
                            .into_iter()
                            .map(|mut error| {
                                error.name = error.name.to_case(Case::Camel);
                                error.fields = error.fields.into_iter().map(camel_case_field).collect();
                                error
                            })
                            .collect();
                    }
                    endpoint
                })
                .collect();

            gen_service
        })
        .collect();

    let formatted_structs = data
        .structs
        .clone()
        .into_iter()
        .map(|mut struct_element| {
            if struct_element.config.snake_case_fields {
                struct_element.inner = match struct_element.inner {
                    Type::Struct { name, fields } => {
                        Type::struct_(name, fields.into_iter().map(camel_case_field).collect())
                    }
                    _ => unreachable!(),
                };

                struct_element
            } else {
                struct_element
            }
        })
        .collect();

    Data {
        project_name: data.project_name.clone(),
        project_root: data.project_root.clone(),
        output_dir: data.output_dir.clone(),
        services: formatted_services,
        enums: data.enums.clone(),
        structs: formatted_structs,
        error_codes: data.error_codes.clone(),
    }
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

/// Returns one violation string per missing/blank description in the
/// definition. Endpoint descriptions become MCP tool descriptions and doc
/// text; enum variant descriptions are emitted into the generated JSON
/// schemas — both are validated.
fn description_violations(definition: &Definition, path: &Path) -> Vec<String> {
    fn blank(s: &str) -> bool {
        s.trim().is_empty()
    }

    fn check_enum(inner: &Type, path: &Path, violations: &mut Vec<String>) {
        if let Type::Enum { name, variants } = inner {
            for variant in variants {
                if blank(&variant.description) {
                    violations.push(format!(
                        "{}: enum '{}' variant '{}': missing or empty description",
                        path.display(),
                        name,
                        variant.name
                    ));
                }
            }
        }
    }

    let mut violations = vec![];
    match definition {
        Definition::EndpointSchema(def) => {
            if blank(&def.schema.schema.description) {
                violations.push(format!(
                    "{}: service '{}' endpoint '{}': missing or empty description",
                    path.display(),
                    def.service_name,
                    def.schema.schema.name
                ));
            }
        }
        Definition::EndpointSchemaList(def) => {
            for endpoint in &def.endpoints {
                if blank(&endpoint.schema.description) {
                    violations.push(format!(
                        "{}: service '{}' endpoint '{}': missing or empty description",
                        path.display(),
                        def.service_name,
                        endpoint.schema.name
                    ));
                }
            }
        }
        Definition::Enum(element) => check_enum(&element.inner, path, &mut violations),
        Definition::EnumList(list) => {
            for element in &list.enum_elements {
                check_enum(&element.inner, path, &mut violations);
            }
        }
        Definition::ErrorCodeList(list) => {
            // Error-code descriptions are not cosmetic either: they become the doc
            // comments on the generated `EnumErrorCode` variants and the third
            // column of docs/error_codes/error_codes.md. A blank one produces an
            // error a caller cannot interpret.
            for code in &list.codes {
                if blank(&code.description) {
                    violations.push(format!(
                        "{}: error code '{}' ({}): missing or empty description",
                        path.display(),
                        code.name,
                        code.code
                    ));
                }
            }
        }
        // Struct fields cannot carry RON descriptions — Field.description is
        // #[serde(skip)] upstream — so struct definitions can never violate.
        Definition::Struct(_) | Definition::StructList(_) => {}
    }
    violations
}

fn process_input_files(dir: PathBuf, allow_empty_descriptions: bool) -> eyre::Result<Vec<Definition>> {
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
    let mut config_errors = vec![];
    let mut description_errors = vec![];
    for path in paths {
        match process_file(path.as_path()) {
            Ok(rust_config) => {
                if let Some(config) = rust_config {
                    if !allow_empty_descriptions {
                        description_errors.extend(description_violations(&config, path.as_path()));
                    }
                    rust_configs.push(config);
                    valid_config_files_counter += 1;
                }
            }
            Err(err) => match path.file_name() {
                Some(name) if name.to_str().unwrap() == "version.toml" => (),
                Some(_) => config_errors.push(format!("{path:?}: {err}")),
                None => (),
            },
        }
    }

    if !config_errors.is_empty() {
        bail!("Error processing RON config files:\n{}", config_errors.join("\n"));
    }

    if !description_errors.is_empty() {
        bail!(
            "Empty-description validation failed for {} item(s). Every endpoint, enum variant \
             and error code needs a description (these become MCP tool metadata, generated doc \
             comments and the error-code reference). Pass --allow-empty-descriptions to \
             bypass:\n{}",
            description_errors.len(),
            description_errors.join("\n")
        );
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
    error_codes: Vec<ErrorCodeSchema>,
}

fn build_object_lists(dir: PathBuf, allow_empty_descriptions: bool) -> eyre::Result<InputObjects> {
    let rust_configs = process_input_files(dir, allow_empty_descriptions)?;

    let mut service_schema_map: HashMap<(String, u16), Vec<EndpointSchemaElement>> = HashMap::new();

    let mut services: Vec<GenService> = vec![];

    let mut enums: Vec<EnumElement> = vec![];
    let mut structs: Vec<StructElement> = vec![];
    let mut custom_error_codes: Vec<ErrorCodeSchema> = vec![];

    for config in rust_configs {
        match config {
            Definition::EndpointSchema(schema_definition) => service_schema_map
                .entry((schema_definition.service_name, schema_definition.service_id))
                .or_default()
                .push(schema_definition.schema),
            Definition::EndpointSchemaList(schema_list_definition) => service_schema_map
                .entry((schema_list_definition.service_name, schema_list_definition.service_id))
                .or_default()
                .extend(schema_list_definition.endpoints.into_iter().map(|mut ele| {
                    if !ele.config.override_parent {
                        ele.config = schema_list_definition.config.clone();
                    }

                    ele
                })),
            Definition::Enum(enum_type) => enums.push(enum_type),
            Definition::EnumList(enums_definition) => {
                enums.extend(enums_definition.enum_elements.into_iter().map(|mut ele| {
                    if !ele.config.override_parent {
                        ele.config = enums_definition.config.clone();
                    }

                    ele
                }))
            }
            Definition::ErrorCodeList(error_code_list) => custom_error_codes.extend(error_code_list.codes),
            Definition::Struct(struct_element) => structs.push(struct_element),
            Definition::StructList(structs_definition) => {
                structs.extend(structs_definition.struct_elements.into_iter().map(|mut ele| {
                    if !ele.config.override_parent {
                        ele.config = structs_definition.config.clone();
                    }

                    ele
                }))
            }
        }
    }

    if !service_schema_map.is_empty() {
        for ((service_name, service_id), endpoint_schemas) in service_schema_map {
            services.push(GenService::new(service_name, service_id, endpoint_schemas));
        }
    }

    // Sort services by (id, name), not id alone.
    //
    // `service_schema_map` is a HashMap, so services come out in an order that
    // is randomised per process. Sorting by `id` is stable but not a *total*
    // order: service ids are not unique (pays.online-backend has seven services
    // at id 1), so every run tied services in a different order and every
    // regeneration produced a spurious diff in docs/services.json and
    // docs/README.md. Adding `name` as a tiebreak makes output deterministic,
    // which is what lets `--check` be trusted in CI.
    services.sort_by(|a, b| a.id.cmp(&b.id).then_with(|| a.name.cmp(&b.name)));

    // Sort the endpoints of each service by their codes
    services
        .iter_mut()
        .for_each(|service| service.endpoints.sort_by_key(|a| a.schema.code));

    // Sort enums and structs by their default ordering
    enums.sort();
    structs.sort();

    let error_codes = build_error_code_catalog(custom_error_codes)?;
    validate_reserved_enum_names(&enums)?;
    validate_endpoint_error_codes(&services, &error_codes)?;

    Ok(InputObjects {
        services,
        enums,
        structs,
        error_codes,
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

    // The version of endpoint-libs that we require - dynamically fetched from Cargo.toml metadata
    let libs_version_requirement = env!("ENDPOINT_LIBS_REQUIREMENT");

    let libs_version_req = VersionReq::parse(libs_version_requirement).unwrap();

    let caller_libs_version = Version::parse(&version_config.libs.version).unwrap();

    if !binary_version_req.matches(&current_crate_version) {
        Err(eyre!(
            "Binary version constraint not satisfied. Version: {} is specified in version.toml. Current binary version is: {}",
            &version_config.binary.version,
            &get_crate_version()
        ))
    } else if !libs_version_req.matches(&caller_libs_version) {
        Err(eyre!(
            "endpoint-libs version constraint not satisfied. Version: {} is specified in version.toml. This version of endpoint-gen requires: {}",
            caller_libs_version,
            libs_version_requirement
        ))
    } else {
        Ok(())
    }
}

fn get_crate_version() -> &'static str {
    // Get the crate version from the Cargo.toml at compile time
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;
    use endpoint_gen::definitions::RustGenConfig;
    use endpoint_libs::model::{EndpointErrorCodeRef, EndpointErrorSchema, EndpointSchema, Field};

    #[test]
    fn format_for_docs_camel_cases_endpoint_error_fields() {
        let data = Data {
            project_name: "test".into(),
            project_root: PathBuf::new(),
            output_dir: PathBuf::new(),
            services: vec![GenService::new(
                "test_service".to_string(),
                1,
                vec![EndpointSchemaElement {
                    frontend_facing: true,
                    config: RustGenConfig {
                        snake_case_fields: true,
                        ..Default::default()
                    },
                    schema: EndpointSchema::new(
                        "Login",
                        10001,
                        vec![Field::new("user_name", Type::String)],
                        vec![Field::new("access_token", Type::String)],
                    )
                    .with_errors(vec![
                        EndpointErrorSchema::new("PasswordTooShort", EndpointErrorCodeRef::new("BadRequest"))
                            .with_message("Password too short")
                            .with_fields(vec![
                                Field::new("min_length", Type::Int32),
                                Field::new("actual_length", Type::Int32),
                            ]),
                    ]),
                }],
            )],
            enums: vec![],
            structs: vec![],
            error_codes: vec![],
        };

        let docs = format_for_docs(&data);
        let endpoint = &docs.services[0].endpoints[0].schema;

        assert_eq!(endpoint.parameters[0].name, "userName");
        assert_eq!(endpoint.returns[0].name, "accessToken");
        assert_eq!(endpoint.errors[0].fields[0].name, "minLength");
        assert_eq!(endpoint.errors[0].fields[1].name, "actualLength");
    }

    use endpoint_gen::definitions::EndpointSchemaListDefinition;
    use endpoint_libs::model::EnumVariant;

    fn endpoint_list(descriptions: &[&str]) -> Definition {
        Definition::EndpointSchemaList(EndpointSchemaListDefinition {
            service_name: "userApi".to_string(),
            service_id: 6,
            config: RustGenConfig::default(),
            endpoints: descriptions
                .iter()
                .enumerate()
                .map(|(i, desc)| EndpointSchemaElement {
                    frontend_facing: true,
                    config: RustGenConfig::default(),
                    schema: EndpointSchema::new(format!("Endpoint{i}"), 60000 + i as u32, vec![], vec![])
                        .with_description(*desc),
                })
                .collect(),
        })
    }

    fn enum_definition(variant_descriptions: &[&str]) -> Definition {
        Definition::Enum(EnumElement {
            config: RustGenConfig::default(),
            inner: Type::Enum {
                name: "UserRole".to_string(),
                variants: variant_descriptions
                    .iter()
                    .enumerate()
                    .map(|(i, desc)| {
                        EnumVariant::new_with_description(format!("Variant{i}"), desc.to_string(), i as i64)
                    })
                    .collect(),
            },
        })
    }

    #[test]
    fn description_violations_flags_empty_and_whitespace_endpoints() {
        let path = Path::new("config/schema_lists/060_user/061_user_api.ron");
        let violations = description_violations(&endpoint_list(&["Fetches a profile.", "", "   \t"]), path);
        assert_eq!(violations.len(), 2);
        assert!(violations[0].contains("service 'userApi'"));
        assert!(violations[0].contains("endpoint 'Endpoint1'"));
        assert!(violations[0].contains("061_user_api.ron"));
        assert!(violations[1].contains("endpoint 'Endpoint2'"));
    }

    #[test]
    fn description_violations_passes_documented_endpoints() {
        let path = Path::new("config/a.ron");
        let violations = description_violations(&endpoint_list(&["Documented.", "Also documented."]), path);
        assert!(violations.is_empty());
    }

    #[test]
    fn description_violations_flags_blank_enum_variants() {
        let path = Path::new("config/enums.ron");
        let violations = description_violations(&enum_definition(&["Platform admin", "", " "]), path);
        assert_eq!(violations.len(), 2);
        assert!(violations[0].contains("enum 'UserRole'"));
        assert!(violations[0].contains("variant 'Variant1'"));
        assert!(violations[1].contains("variant 'Variant2'"));
    }

    #[test]
    fn description_violations_ignores_structs() {
        // Struct fields cannot carry RON descriptions (Field.description is
        // serde-skipped upstream), so StructList definitions never violate.
        let path = Path::new("config/structs.ron");
        let def = Definition::StructList(endpoint_gen::definitions::StructListDefinition {
            config: RustGenConfig::default(),
            struct_elements: vec![],
        });
        assert!(description_violations(&def, path).is_empty());
    }

    #[test]
    fn description_violations_flags_blank_error_codes() {
        use endpoint_gen::definitions::{ErrorCodeListDefinition, ErrorCodeSchema};
        let path = Path::new("config/error_codes.ron");
        let def = Definition::ErrorCodeList(ErrorCodeListDefinition {
            codes: vec![
                ErrorCodeSchema::new("BadRequest", 400, "The request was malformed."),
                ErrorCodeSchema::new("Teapot", 418, ""),
                ErrorCodeSchema::new("Blank", 419, "  \t "),
            ],
        });
        let violations = description_violations(&def, path);
        assert_eq!(violations.len(), 2, "{violations:?}");
        assert!(violations[0].contains("error code 'Teapot' (418)"), "{violations:?}");
        assert!(violations[1].contains("error code 'Blank' (419)"), "{violations:?}");
        assert!(violations[0].contains("error_codes.ron"));
    }
}
