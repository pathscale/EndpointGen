use crate::definitions::EnumElement;
use crate::docs::Data;
use convert_case::{Case, Casing};
use endpoint_libs::model::{EndpointErrorSchema, EnumVariant, Type};
use eyre::bail;
use itertools::Itertools;
use std::collections::{BTreeSet, HashMap};
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::process::Command;

pub trait ToRust {
    fn to_rust_ref(&self, serde_with: bool) -> String;
    fn to_rust_decl(&self, serde_with: bool, add_derives: bool) -> String;
    fn add_derives(&self, input: String) -> String;
}

impl ToRust for Type {
    fn to_rust_ref(&self, serde_with: bool) -> String {
        match self {
            Type::UInt32 => "u32".to_owned(),
            Type::Int32 => "i32".to_owned(),
            Type::Int64 => "i64".to_owned(),
            Type::Float64 => "f64".to_owned(),
            Type::TimeStampMs => "i64".to_owned(),
            Type::Struct { name, .. } => name.clone(),
            Type::StructRef(name) => name.clone(),
            Type::Object => "serde_json::Value".to_owned(),
            // Type::DataTable { name, .. } => format!("Vec<{name}>"),
            Type::StructTable { struct_ref } => format!("Vec<{struct_ref}>"),
            Type::Vec(ele) => {
                format!("Vec<{}>", ele.to_rust_ref(serde_with))
            }
            Type::Unit => "()".to_owned(),
            Type::Optional(t) => {
                format!("Option<{}>", t.to_rust_ref(serde_with))
            }
            Type::Boolean => "bool".to_owned(),
            Type::String => "String".to_owned(),
            Type::Bytea => "Vec<u8>".to_owned(),
            Type::UUID => "Uuid".to_owned(),
            Type::NanoId { len } => format!("Nanoid<{len}, Base62Alphabet>"),
            Type::IpAddr => "IpAddr".to_owned(),
            Type::Enum { name, .. } => format!("Enum{}", name.to_case(Case::Pascal),),
            Type::EnumRef { name, prefixed_name } => {
                if *prefixed_name {
                    format!("Enum{}", name.to_case(Case::Pascal),)
                } else {
                    name.to_case(Case::Pascal)
                }
            }
            Type::BlockchainDecimal => "Decimal".to_owned(),
            Type::BlockchainAddress if serde_with => "Address".to_owned(),
            Type::BlockchainTransactionHash if serde_with => "H256".to_owned(),
            Type::BlockchainAddress => "BlockchainAddress".to_owned(),
            Type::BlockchainTransactionHash => "BlockchainTransactionHash".to_owned(),
            // `Type` is #[non_exhaustive] as of endpoint-libs 2.0, so a newer libs
            // release can add variants without breaking this build. Panicking is the
            // right behaviour: emitting Rust for a type we do not understand would
            // produce silently wrong generated code.
            other => panic!(
                "endpoint-gen does not know how to emit Rust for {other:?}; \
                 upgrade endpoint-gen to match your endpoint-libs version"
            ),
        }
    }

    fn to_rust_decl(&self, serde_with: bool, add_derives: bool) -> String {
        match self {
            Type::Struct { name, fields } => {
                let mut fields = fields.iter().map(|x| {
                    let opt = matches!(&x.ty, Type::Optional(_));
                    let serde_with_opt = match &x.ty {
                        Type::BlockchainDecimal => "rust_decimal::serde::str",
                        Type::BlockchainAddress if serde_with => "WithBlockchainAddress",
                        Type::BlockchainTransactionHash if serde_with => "WithBlockchainTransactionHash",
                        // TODO: handle optional decimals
                        // Type::Optional(t) if matches!(**t, Type::BlockchainDecimal) => {
                        //     "WithBlockchainDecimal"
                        // }
                        // Type::Optional(t) if matches!(**t, Type::BlockchainAddress) => {
                        //     "WithBlockchainAddress"
                        // }
                        // Type::Optional(t) if matches!(**t, Type::BlockchainTransactionHash) => {
                        //     "WithBlockchainTransactionHash"
                        // }
                        _ => "",
                    };
                    format!(
                        "{} {} {} pub {}: {}",
                        if opt { "#[serde(default)]" } else { "" },
                        if serde_with_opt.is_empty() {
                            "".to_string()
                        } else {
                            format!("#[serde(with = \"{serde_with_opt}\")]")
                        },
                        wire_rename_attr(&x.name),
                        x.name,
                        x.ty.to_rust_ref(serde_with)
                    )
                });
                let input = format!("pub struct {} {{{}}}", name, fields.join(","));

                if add_derives {
                    self.add_derives(input)
                } else {
                    input
                }
            }
            Type::Enum { name, variants: fields } => {
                let fields = fields
                    .iter()
                    .sorted_by_key(|variant| variant.value)
                    .map(|x| {
                        let variant_name = if x.name.chars().last().unwrap().is_lowercase() {
                            x.name.to_case(Case::Pascal)
                        } else {
                            x.name.clone()
                        };
                        // A blank description must omit the `///` line entirely: an
                        // empty doc comment trips clippy::empty_docs downstream.
                        if x.description.trim().is_empty() {
                            format!(
                                r#"
    {} = {}
"#,
                                variant_name, x.value
                            )
                        } else {
                            format!(
                                r#"
    #[doc = {:?}]
    {} = {}
"#,
                                x.description, variant_name, x.value
                            )
                        }
                    })
                    .collect::<Vec<_>>();
                let enum_content = format!(
                    r#"pub enum Enum{} {{{}}}"#,
                    name.to_case(Case::Pascal),
                    fields.join(",")
                );

                if add_derives {
                    self.add_derives(enum_content)
                } else {
                    enum_content
                }
            }
            x => x.to_rust_ref(serde_with),
        }
    }

    fn add_derives(&self, input: String) -> String {
        match self {
            Self::Enum { .. } => Self::add_default_enum_derives(input),
            Self::Struct { .. } => Self::add_default_struct_derives(input),
            _ => input,
        }
    }
}

pub fn collect_rust_recursive_types(t: Type) -> Vec<Type> {
    match t {
        Type::Struct { ref fields, .. } => {
            let mut v = vec![t.clone()];
            for x in fields {
                v.extend(collect_rust_recursive_types(x.ty.clone()));
            }
            v
        }
        // Type::DataTable { name, fields } => {
        //     collect_rust_recursive_types(Type::struct_(name, fields))
        // }
        // Type::StructTable { struct_ref } => {
        //     collect_rust_recursive_types(Type::struct_ref(struct_ref))
        // }
        Type::Vec(x) => collect_rust_recursive_types(*x),
        Type::Optional(x) => collect_rust_recursive_types(*x),
        _ => vec![],
    }
}

fn endpoint_error_enum_name(endpoint_name: &str) -> String {
    format!("{}Error", endpoint_name.to_case(Case::Pascal))
}

fn endpoint_error_variant_name(error: &EndpointErrorSchema) -> String {
    error.name.to_case(Case::Pascal)
}

pub(crate) fn error_code_variant_name(name: &str) -> String {
    name.to_case(Case::Pascal)
}

fn endpoint_error_code_expr(error: &EndpointErrorSchema) -> String {
    format!("EnumErrorCode::{}", error_code_variant_name(error.code.variant()))
}

fn rust_string_literal(value: &str) -> String {
    serde_json::to_string(value).expect("string serialization should not fail")
}

fn gen_endpoint_error_enum(
    endpoint_name: &str,
    errors: &[EndpointErrorSchema],
    mut writer: impl Write,
) -> eyre::Result<()> {
    if errors.is_empty() {
        return Ok(());
    }

    let enum_name = endpoint_error_enum_name(endpoint_name);
    writeln!(
        writer,
        "#[derive(Serialize, Deserialize, Debug, Clone)]\npub enum {enum_name} {{"
    )?;

    for error in errors {
        let variant_name = endpoint_error_variant_name(error);
        if !error.message.trim().is_empty() {
            writeln!(writer, "    #[doc = {:?}]", error.message)?;
        }
        if error.fields.is_empty() {
            writeln!(writer, "    {variant_name},")?;
        } else {
            let fields = error
                .fields
                .iter()
                .map(|field| format!("{}: {}", field.name, field.ty.to_rust_ref(true)))
                .join(", ");
            writeln!(writer, "    {variant_name} {{ {fields} }},")?;
        }
    }

    writeln!(writer, "}}\n")?;
    writeln!(writer, "impl From<{enum_name}> for CustomError {{")?;
    writeln!(writer, "    fn from(err: {enum_name}) -> Self {{")?;
    writeln!(writer, "        match err {{")?;

    for error in errors {
        let variant_name = endpoint_error_variant_name(error);
        let code_expr = endpoint_error_code_expr(error);
        let message = &error.message;
        let kind = rust_string_literal(&variant_name);
        if error.fields.is_empty() {
            writeln!(
                writer,
                "            {enum_name}::{variant_name} => CustomError::new({code_expr}).with_message({}).with_kind({kind}),",
                rust_string_literal(message),
            )?;
        } else {
            let field_names = error.fields.iter().map(|field| field.name.as_str()).join(", ");
            let json_fields = error
                .fields
                .iter()
                .map(|field| format!(r#""{}": {}"#, field.name.to_case(Case::Camel), field.name))
                .join(", ");
            writeln!(
                writer,
                "            {enum_name}::{variant_name} {{ {field_names} }} => CustomError::new({code_expr}).with_message({}).with_kind({kind}).with_details(serde_json::json!({{ {json_fields} }})),",
                rust_string_literal(message),
            )?;
        }
    }

    writeln!(writer, "        }}")?;
    writeln!(writer, "    }}")?;
    writeln!(writer, "}}\n")?;

    Ok(())
}

pub fn gen_model_rs(data: &Data) -> eyre::Result<()> {
    let db_filename = data.output_dir.join("model.rs");

    // Ensure the parent directories exist
    if let Some(parent) = db_filename.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let worktable_imports = if data.enums.iter().any(|e| e.config.worktable_support)
        || data.structs.iter().any(|s| s.config.worktable_support)
    {
        r#"use worktable::prelude::*;
           use rkyv::Archive;
        "#
    } else {
        ""
    };

    let json_schema_imports = if data.enums.iter().any(|e| e.config.json_schema_gen)
        || data.structs.iter().any(|s| s.config.json_schema_gen)
    {
        r#"use schemars::{schema_for, JsonSchema};"#
    } else {
        ""
    };

    // Only a schema with a UUID field needs the uuid crate; the fleet has none,
    // and an unconditional import forced every consumer to keep the dependency.
    let uuid_import = if data.services.iter().flat_map(|s| &s.endpoints).any(|e| {
        e.schema
            .parameters
            .iter()
            .chain(&e.schema.returns)
            .any(|f| uses_uuid(&f.ty))
            || e.schema.stream_response.as_ref().is_some_and(uses_uuid)
    }) || data.structs.iter().any(|s| uses_uuid(&s.inner))
    {
        "use uuid::Uuid;"
    } else {
        ""
    };

    let mut model_file = File::create(&db_filename)?;
    write!(
        &mut model_file,
        "use endpoint_libs::libs::error_code::ErrorCode;
        use endpoint_libs::libs::ws::*;
        use endpoint_libs::libs::types::*;
        use endpoint_libs::libs::ws::toolbox::CustomError;
        use num_derive::FromPrimitive;
        use serde::*;
        use strum_macros::{{Display, EnumString}};
        {uuid_import}
        use psc_nanoid::{{Nanoid, alphabet::Base62Alphabet}};
        use std::net::IpAddr;
        {worktable_imports}
        {json_schema_imports}
        ",
    )?;

    for e in &data.enums {
        writeln!(&mut model_file, "{}", e.to_rust_decl(false, true))?;
    }
    for s in &data.structs {
        writeln!(&mut model_file, "{}", s.to_rust_decl(false, true))?;
    }
    check_endpoint_codes(data, &mut model_file)?;
    dump_endpoint_schema(data, &mut model_file)?;
    dump_type_registry(data, &mut model_file)?;

    let enum_ = Type::enum_(
        "ErrorCode",
        data.error_codes
            .iter()
            .map(|x| EnumVariant::new_with_description(error_code_variant_name(&x.name), x.description.clone(), x.code))
            .collect(),
    );
    writeln!(&mut model_file, "{}", enum_.to_rust_decl(false, true))?;
    writeln!(
        &mut model_file,
        r#"
impl From<EnumErrorCode> for ErrorCode {{
    fn from(e: EnumErrorCode) -> Self {{
        ErrorCode::new(e as _)
    }}
}}
    "#
    )?;

    let mut endpoint_reqres_types = BTreeSet::new();
    for s in &data.services {
        for e in &s.endpoints {
            let req = Type::struct_(format!("{}Request", e.schema.name), e.schema.parameters.clone());
            let resp = Type::struct_(format!("{}Response", e.schema.name), e.schema.returns.clone());
            endpoint_reqres_types.extend(
                [
                    collect_rust_recursive_types(req),
                    collect_rust_recursive_types(resp),
                    e.schema
                        .stream_response
                        .clone()
                        .into_iter()
                        .flat_map(Type::try_unwrap)
                        .collect::<Vec<_>>(),
                    e.schema
                        .errors
                        .iter()
                        .flat_map(|error| {
                            error
                                .fields
                                .iter()
                                .flat_map(|field| collect_rust_recursive_types(field.ty.clone()))
                        })
                        .collect::<Vec<_>>(),
                ]
                .concat(),
            );
        }
    }
    for s in endpoint_reqres_types {
        write!(&mut model_file, r#"{}"#, s.to_rust_decl(true, true))?;
    }

    for s in &data.services {
        for endpoint in &s.endpoints {
            gen_endpoint_error_enum(&endpoint.schema.name, &endpoint.schema.errors, &mut model_file)?;
        }
    }

    for s in &data.services {
        for endpoint in &s.endpoints {
            let roles_list = resolve_roles_ids(&endpoint.schema.roles, &data.enums)?
                .into_iter()
                .map(|x| x.to_string())
                .join(", ");

            write!(
                &mut model_file,
                "
impl WsRequest for {end_name2}Request {{
    type Response = {end_name2}Response;
    const METHOD_ID: u32 = {code};
    const ROLES: &[u32] = &[{roles_list}];
    const SCHEMA: &'static str = {schema};
}}
impl WsResponse for {end_name2}Response {{
    type Request = {end_name2}Request;
}}
",
                end_name2 = endpoint.schema.name.to_case(Case::Pascal),
                code = endpoint.schema.code,
                schema = format!("{:?}", serde_json::to_string_pretty(&endpoint.schema)?)
            )?;
        }
    }
    model_file.flush()?;
    drop(model_file);
    rustfmt(&db_filename)?;

    Ok(())
}

/// Resolves the IDs of roles from a list of role names and a list of enum types.
/// endpoint_roles: vec!["Role1::Value1", "Role1::Value2"]
fn resolve_roles_ids(endpoint_roles: &Vec<String>, all_enums: &Vec<EnumElement>) -> eyre::Result<Vec<i64>> {
    let mut all_enums_typed: HashMap<String, Vec<EnumVariant>> = HashMap::new();
    for e in all_enums {
        if let Type::Enum { name: _, variants } = &e.inner {
            all_enums_typed.insert(e.to_rust_ref(false), variants.clone());
        }
    }

    let mut roles_ids = vec![];
    for role in endpoint_roles {
        let (role_enum_name, role_variant_name) = role.split_once("::").unwrap_or(("", role.as_str()));

        if let Some(role_enum_variants) = all_enums_typed.get(role_enum_name) {
            if let Some(role_variant_in_endpoint) = role_enum_variants.iter().find(|v| v.name == role_variant_name) {
                roles_ids.push(role_variant_in_endpoint.value);
            } else {
                bail!("Role variant '{role_variant_name}' not found in enum '{role_enum_name}'");
            }
        } else {
            bail!("Role enum '{role_enum_name}' not found");
        }
    }
    // Reject duplicate role IDs instead of silently deduplicating them.
    let mut roles_ids_set: BTreeSet<i64> = BTreeSet::new();
    for id in &roles_ids {
        if !roles_ids_set.insert(*id) {
            bail!("Duplicate role ID found: {id}");
        }
    }

    Ok(roles_ids_set.into_iter().collect())
}

pub fn rustfmt(f: &Path) -> eyre::Result<()> {
    let exit = Command::new("rustfmt")
        .arg("--edition")
        .arg("2021")
        .arg(f)
        .spawn()?
        .wait()?;
    if !exit.success() {
        bail!("failed to rustfmt {:?}", exit);
    }
    Ok(())
}

pub fn check_endpoint_codes(data: &Data, mut writer: impl Write) -> eyre::Result<()> {
    let mut variants = vec![];
    for s in &data.services {
        for e in &s.endpoints {
            variants.push(EnumVariant::new(e.schema.name.clone(), e.schema.code as _));
        }
    }
    let enum_ = Type::enum_("Endpoint", variants);
    writeln!(writer, "{}", enum_.to_rust_decl(false, true))?;
    // if it compiles, there're no duplicate codes or names
    Ok(())
}
pub fn dump_endpoint_schema(data: &Data, mut writer: impl Write) -> eyre::Result<()> {
    let mut cases = vec![];
    for s in &data.services {
        for e in &s.endpoints {
            cases.push(format!(
                "Self::{name} => {name}Request::SCHEMA,",
                name = e.schema.name.to_case(Case::Pascal),
            ));
        }
    }
    let code = format!(
        r#"
    impl EnumEndpoint {{
        pub fn schema(&self) -> endpoint_libs::model::EndpointSchema {{
            let schema = match self {{
                {cases}
            }};
            serde_json::from_str(schema).unwrap()
        }}
    }}
    "#,
        cases = cases.join("\n")
    );
    writeln!(writer, "{code}")?;
    Ok(())
}

/// Collects every shared type definition (structs, enums, the generated
/// `ErrorCode` enum) that endpoint schemas may reference by name.
pub fn shared_type_definitions(data: &Data) -> Vec<Type> {
    let mut types: Vec<Type> = data.structs.iter().map(|s| s.inner.clone()).collect();
    types.extend(data.enums.iter().map(|e| e.inner.clone()));
    types.push(Type::enum_(
        "ErrorCode",
        data.error_codes
            .iter()
            .map(|x| EnumVariant::new_with_description(error_code_variant_name(&x.name), x.description.clone(), x.code))
            .collect(),
    ));
    types
}

/// Emits `TYPE_DEFINITIONS` and `type_registry()` into the generated model:
/// the full list of shared type definitions serialized as JSON (same embedding
/// pattern as `WsRequest::SCHEMA`), plus a helper that deserializes them into
/// an `endpoint_libs::model::TypeRegistry` for `WebsocketServer::enable_mcp()`.
pub fn dump_type_registry(data: &Data, mut writer: impl Write) -> eyre::Result<()> {
    let types = shared_type_definitions(data);
    let serialized = format!("{:?}", serde_json::to_string_pretty(&types)?);
    let code = format!(
        r##"
/// JSON-serialized shared struct/enum definitions referenced by endpoint schemas.
pub const TYPE_DEFINITIONS: &'static str = {serialized};

/// Builds the type registry over all shared definitions, for use with
/// `WebsocketServer::enable_mcp()`.
pub fn type_registry() -> endpoint_libs::model::TypeRegistry {{
    let types: Vec<endpoint_libs::model::Type> =
        serde_json::from_str(TYPE_DEFINITIONS).expect("Invalid embedded type definitions");
    let mut registry = endpoint_libs::model::TypeRegistry::new();
    registry.add_all(types.iter());
    registry
}}
"##
    );
    writeln!(writer, "{code}")?;
    Ok(())
}

/// A `#[serde(rename)]` for a snake_case field whose documented camelCase
/// name serde would not produce by itself.
///
/// The docs and `services.json` camelCase field names with `convert_case`,
/// which splits a word at a digit: `login_failures_count_24h` is documented as
/// `loginFailuresCount24H`. The generated structs derive their wire names with
/// `#[serde(rename_all = "camelCase")]`, which does not: serde sends
/// `loginFailuresCount24h`. A frontend reading the documented name then reads
/// nothing. Where the two disagree, the field is renamed to the documented
/// name, so the wire matches the contract frontends are generated from.
pub fn wire_rename_attr(field: &str) -> String {
    if !field.contains('_') {
        return String::new();
    }
    let documented = field.to_case(Case::Camel);
    if documented == serde_camel_case(field) {
        String::new()
    } else {
        format!("#[serde(rename = \"{documented}\")]")
    }
}

/// What `#[serde(rename_all = "camelCase")]` makes of a snake_case field.
fn serde_camel_case(field: &str) -> String {
    let mut out = String::with_capacity(field.len());
    let mut upper = false;
    for c in field.chars() {
        if c == '_' {
            upper = true;
        } else if upper {
            out.extend(c.to_uppercase());
            upper = false;
        } else {
            out.push(c);
        }
    }
    out
}

/// Whether `ty` is or contains a `UUID`.
fn uses_uuid(ty: &Type) -> bool {
    match ty {
        Type::UUID => true,
        Type::Optional(inner) | Type::Vec(inner) => uses_uuid(inner),
        Type::Struct { fields, .. } => fields.iter().any(|f| uses_uuid(&f.ty)),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::definitions::{EndpointSchemaElement, RustGenConfig};
    use endpoint_libs::model::{EndpointSchema, Field};

    #[test]
    fn enum_decl_omits_doc_comment_for_blank_descriptions() {
        let e = Type::enum_(
            "sample",
            vec![
                EnumVariant::new_with_description("Documented", "Has docs.", 0),
                EnumVariant::new("Bare", 1),
                EnumVariant::new_with_description("Blank", "   ", 2),
            ],
        );
        let decl = e.to_rust_decl(false, false);
        let lines = decl.lines().map(str::trim).collect::<Vec<_>>();
        assert!(
            lines
                .windows(2)
                .any(|pair| { pair[0] == r#"#[doc = "Has docs."]"# && pair[1] == "Documented = 0" }),
            "documented variant should have its doc attribute immediately before it:\n{decl}"
        );
        let doc_attributes = lines
            .iter()
            .filter(|line| line.starts_with("#[doc ="))
            .collect::<Vec<_>>();
        assert_eq!(
            doc_attributes.len(),
            1,
            "only the documented variant should have a doc attribute:\n{decl}"
        );
        assert!(
            !decl.lines().any(|l| l.trim() == "///"),
            "empty doc comment emitted (clippy::empty_docs):\n{decl}"
        );
        assert!(decl.contains("Bare = 1"));
        assert!(decl.contains("Blank = 2"));
    }

    fn test_data() -> Data {
        let user_info = Type::struct_(
            "UserInfo",
            vec![
                Field::new("user_id", Type::Int64),
                Field::new("role", Type::enum_ref("role", true)),
            ],
        );
        let role = Type::enum_("role", vec![EnumVariant::new("Admin", 1)]);
        Data {
            project_name: "api.example.com".into(),
            project_root: std::path::PathBuf::new(),
            output_dir: std::path::PathBuf::new(),
            services: vec![crate::definitions::GenService::new(
                "user".to_string(),
                1,
                vec![EndpointSchemaElement {
                    frontend_facing: true,
                    config: RustGenConfig::default(),
                    schema: EndpointSchema::new(
                        "UserGetProfile",
                        10010,
                        vec![Field::new("user_id", Type::Int64)],
                        vec![Field::new("profile", Type::struct_ref("UserInfo"))],
                    )
                    .with_description("Fetches a user profile."),
                }],
            )],
            enums: vec![crate::definitions::EnumElement {
                config: RustGenConfig::default(),
                inner: role,
            }],
            structs: vec![crate::definitions::StructElement {
                config: RustGenConfig::default(),
                inner: user_info,
            }],
            error_codes: vec![],
        }
    }

    #[test]
    fn type_registry_dump_round_trips() {
        let data = test_data();
        let mut out = Vec::new();
        dump_type_registry(&data, &mut out).unwrap();
        let code = String::from_utf8(out).unwrap();

        assert!(code.contains("pub const TYPE_DEFINITIONS"));
        assert!(code.contains("pub fn type_registry()"));

        // Check the escaped literal and round-trip the serialized payload.
        let serialized = serde_json::to_string_pretty(&shared_type_definitions(&data)).unwrap();
        assert!(code.contains(&format!("pub const TYPE_DEFINITIONS: &'static str = {serialized:?};")));
        let types: Vec<Type> = serde_json::from_str(&serialized).unwrap();
        // UserInfo struct + role enum + ErrorCode enum
        assert_eq!(types.len(), 3);

        let mut registry = endpoint_libs::model::TypeRegistry::new();
        registry.add_all(types.iter());
        assert!(registry.get_struct("UserInfo").is_some());
        assert!(registry.get_enum("role").is_some());
        assert!(registry.get_enum("ErrorCode").is_some());

        // The registry must be able to resolve the endpoint's schemas.
        let schema = &data.services[0].endpoints[0].schema;
        let input = schema.to_mcp_input_schema(&registry).unwrap();
        assert_eq!(input["required"], serde_json::json!(["userId"]));
        let output = schema.to_mcp_output_schema(&registry).unwrap();
        assert!(output["$defs"]["UserInfo"].is_object());
    }

    #[test]
    fn enum_decl_sorts_by_signed_discriminant() {
        let e = Type::enum_(
            "sample",
            vec![
                EnumVariant::new_with_description("High", "default = 0", 10),
                EnumVariant::new("Near", -1),
                EnumVariant::new("Low", 2),
                EnumVariant::new("Far", -9),
                EnumVariant::new("Tie", 2),
            ],
        );
        let decl = e.to_rust_decl(false, false);
        let declarations = decl
            .lines()
            .filter(|line| line.contains(" = ") && !line.trim_start().starts_with('#'))
            .map(str::trim)
            .collect::<Vec<_>>();

        assert_eq!(
            declarations,
            ["Far = -9", "Near = -1", "Low = 2", "Tie = 2", "High = 10"]
        );
        assert!(decl.contains("#[doc = \"default = 0\"]"));
    }

    #[test]
    fn endpoint_error_docs_escape_text_and_omit_blank_messages() {
        use endpoint_libs::model::{EndpointErrorCodeRef, EndpointErrorSchema};

        let message = "first line\nsecond \"quoted\" \\ path NAKED_ERROR_TOKEN";
        let errors = vec![
            EndpointErrorSchema::new("MultiLine", EndpointErrorCodeRef::new("BadRequest")).with_message(message),
            EndpointErrorSchema::new("WithFields", EndpointErrorCodeRef::new("BadRequest"))
                .with_message(message)
                .with_fields(vec![Field::new("detail_code", Type::String)]),
            EndpointErrorSchema::new("Empty", EndpointErrorCodeRef::new("BadRequest")).with_message(""),
            EndpointErrorSchema::new("Whitespace", EndpointErrorCodeRef::new("BadRequest")).with_message(" \t\n "),
        ];
        let mut output = Vec::new();
        gen_endpoint_error_enum("UserLogin", &errors, &mut output).unwrap();
        let generated = String::from_utf8(output).unwrap();
        let lines = generated.lines().map(str::trim).collect::<Vec<_>>();
        let expected = format!("#[doc = {:?}]", message);
        assert!(lines
            .windows(2)
            .any(|pair| pair[0] == expected && pair[1] == "MultiLine,"));
        assert!(lines
            .windows(2)
            .any(|pair| { pair[0] == expected && pair[1] == "WithFields { detail_code: String }," }));
        assert!(
            !lines
                .iter()
                .any(|line| line.starts_with("second \"quoted\" \\ path NAKED_ERROR_TOKEN")),
            "second-line error text escaped its attribute:\n{generated}"
        );
        assert_eq!(lines.iter().filter(|line| line.starts_with("#[doc =")).count(), 2);
        assert!(generated.contains("with_message(\"first line\\nsecond \\\"quoted\\\" \\\\ path NAKED_ERROR_TOKEN\")"));
        assert!(generated.contains("with_message(\" \\t\\n \")"));
        assert!(generated.contains("EnumErrorCode::BadRequest"));
        assert!(generated.contains("with_kind(\"MultiLine\")"));
        assert!(generated.contains("with_details(serde_json::json!({ \"detailCode\": detail_code }))"));
    }
}
