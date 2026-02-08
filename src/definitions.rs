use crate::{DefinitionVariant, rust::ToRust};
use convert_case::{Case, Casing};
use endpoint_libs::model::{EndpointSchema, Type};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;
use smart_serde_default::smart_serde_default;

/// Marker trait for types that can be used as Definition variants
/// All types used in Definition must implement this to ensure they are properly validatable
pub trait DefinitionVariant: GenElement<Self> {}

#[derive(Serialize, Deserialize)]
pub enum Definition {
    EndpointSchema(EndpointSchemaDefinition),
    EndpointSchemaList(EndpointSchemaListDefinition),
    Enum(EnumElement),
    EnumList(Vec<EnumElement>),
    Struct(StructElement),
    StructList(Vec<StructElement>),
}

impl Definition {
    pub fn validate_self(&self) -> eyre::Result<()> {
        match self {
            Definition::Enum(e) => e.validate_element(),
            Definition::EnumList(list) => {
                for item in list {
                    item.validate_element()?;
                }
                Ok(())
            }
            Definition::Struct(s) => s.validate_element(),
            Definition::StructList(list) => {
                for item in list {
                    item.validate_element()?;
                }
                Ok(())
            }
            Definition::EndpointSchema(schema) => schema.validate_element(),
            Definition::EndpointSchemaList(schemas) => schemas.validate_element(),
        }
    }
}

pub trait GenElement<T: ?Sized>
where
    T: GenElement<T>,
{
    fn validate_element(&self) -> eyre::Result<()>;
}

/// Wraps the [Type::Enum] variant with extra config
#[derive(
    Clone, Debug, Serialize, Deserialize, Hash, PartialEq, PartialOrd, Eq, Ord, DefinitionVariant,
)]
pub struct EnumElement {
    #[serde(default)]
    pub config: RustGenConfig,
    pub inner: Type,
}

impl GenElement<EnumElement> for EnumElement {
    fn validate_element(&self) -> eyre::Result<()> {
        match &self.inner {
            Type::Enum { .. } => Ok(()),
            _ => eyre::bail!("Expected enum type"),
        }
    }
}

impl ToRust for EnumElement {
    fn to_rust_ref(&self, _serde_with: bool) -> String {
        self.validate_element()
            .expect(&format!("EnumElement is invalid: {self:?}"));

        let name = match &self.inner {
            Type::Enum { name, .. } => name.to_case(Case::Pascal),
            _ => unreachable!("The previous validation ensured that this type is a valid Enum"),
        };

        let name = if self.config.prefix_enum {
            format!("Enum{name}")
        } else {
            name
        };

        name
    }

    fn to_rust_decl(&self, serde_with: bool) -> String {
        self.validate_element()
            .expect(&format!("EnumElement is invalid: {self:?}"));

        let code_regex =
            regex::Regex::new(r"=\s*(\d+)").expect("Error building regex to extract endpoint code");

        match &self.inner {
            Type::Enum {
                name: _,
                variants: fields,
            } => {
                let mut fields = fields
                    .iter()
                    .map(|x| {
                        format!(
                            r#"
    /// {}
    {} = {}
"#,
                            x.description,
                            if x.name.chars().last().unwrap().is_lowercase() {
                                x.name.to_case(Case::Pascal)
                            } else {
                                x.name.clone()
                            },
                            x.value
                        )
                    })
                    .sorted_by(|a, b| {
                        // Sort by the endpoint code
                        let code_a = {
                            match code_regex.captures(a) {
                                Some(code) => code[1].parse::<u64>().unwrap_or_else(|err| {
                                    eprintln!(
                                        "Sorting error: {err}: Rust output may not be sorted correctly"
                                    );
                                    0
                                }),
                                None => {
                                    eprintln!(
                                        "Sorting error: Rust output may not be sorted correctly"
                                    );
                                    0
                                }
                            }
                        };

                        let code_b = {
                            match code_regex.captures(b) {
                                Some(code) => {
                                    code[1].parse::<u64>().unwrap_or_else(|err| {
                                        eprintln!(
                                        "Sorting error: {err}: Rust output may not be sorted correctly"
                                    );
                                        0
                                    })
                                }
                                None => {
                                    eprintln!(
                                        "Sorting error: Rust output may not be sorted correctly"
                                    );
                                    0
                                }
                            }
                        };

                        code_a.cmp(&code_b)
                    });
                let enum_content = format!(
                    r#"pub enum {} {{{}}}"#,
                    self.to_rust_ref(serde_with),
                    fields.join(",")
                );

                self.add_derives(enum_content)
            }
            _ => unreachable!(),
        }
    }

    fn add_derives(&self, input: String) -> String {
        if self.config.worktable_support {
            format!(
                r#"#[derive(
                    MemStat,
                    Archive,
                    Clone,
                    Copy,
                    Debug,
                    Display,
                    PartialEq,
                    PartialOrd,
                    Eq,
                    Hash,
                    Ord,
                    EnumString,
                    rkyv::Deserialize,
                    rkyv::Serialize,
                    serde::Serialize,
                    serde::Deserialize,
                )]
                #[rkyv(compare(PartialEq), derive(Debug))]
                #[repr(u8)]
                {input}
            "#
            )
        } else {
            Type::add_default_enum_derives(input)
        }
    }
}

/// Wraps the [Type::Struct] variant with extra config
#[derive(
    Clone, Debug, Serialize, Deserialize, Hash, PartialEq, PartialOrd, Eq, Ord, DefinitionVariant,
)]
pub struct StructElement {
    #[serde(default)]
    pub config: RustGenConfig,
    pub inner: Type,
}

impl GenElement<StructElement> for StructElement {
    fn validate_element(&self) -> eyre::Result<()> {
        match &self.inner {
            Type::Struct { .. } => Ok(()),
            _ => eyre::bail!("Expected struct type"),
        }
    }
}

impl ToRust for StructElement {
    fn to_rust_ref(&self, _serde_with: bool) -> String {
        self.validate_element()
            .expect(&format!("StructElement is invalid: {self:?}"));

        match &self.inner {
            Type::Struct { name, .. } => name.clone(),
            _ => unreachable!("The previous validation ensured that this type is a valid Struct"),
        }
    }

    fn to_rust_decl(&self, serde_with: bool) -> String {
        self.validate_element()
            .expect(&format!("StructElement is invalid: {self:?}"));

        let (name, fields) = match &self.inner {
            Type::Struct { name, fields } => (name.to_case(Case::Pascal), fields),
            _ => unreachable!("The previous validation ensured that this type is a valid Struct"),
        };

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
                "{} {} pub {}: {}",
                if opt { "#[serde(default)]" } else { "" },
                if serde_with_opt.is_empty() {
                    "".to_string()
                } else {
                    format!("#[serde(with = \"{serde_with_opt}\")]")
                },
                x.name,
                x.ty.to_rust_ref(serde_with)
            )
        });
        let input = format!("pub struct {} {{{}}}", name, fields.join(","));

        self.add_derives(input)
    }

    fn add_derives(&self, input: String) -> String {
        if self.config.worktable_support {
            format!(
                r#"#[derive(
                        Clone,
                        Copy,
                        Debug,
                        Default,
                        Eq,
                        Hash,
                        Ord,
                        PartialEq,
                        PartialOrd,
                        derive_more::Display,
                        derive_more::From,
                        derive_more::FromStr,
                        derive_more::Into,
                        MemStat,
                        rkyv::Archive,
                        SizeMeasure,
                        rkyv::Deserialize,
                        rkyv::Serialize,
                        serde::Serialize,
                        serde::Deserialize,
                    )]
                    #[rkyv(compare(PartialEq), derive(Debug, PartialOrd, PartialEq, Eq, Ord))]
                    {input}
                "#
            )
        } else {
            Type::add_default_struct_derives(input)
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Hash, PartialEq, PartialOrd, Eq, Ord, Default)]
pub struct RustGenConfig {
    #[serde(default)]
    prefix_enum: bool,
    #[serde(default)]
    pub worktable_support: bool,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct GenService {
    pub name: String,
    pub id: u16,
    pub endpoints: Vec<EndpointSchemaElement>,
}

impl GenService {
    pub fn new(name: String, id: u16, endpoints: Vec<EndpointSchemaElement>) -> Self {
        Self {
            name,
            id,
            endpoints,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct EndpointSchemaDefinition {
    pub service_name: String,
    pub service_id: u16,
    pub schema: EndpointSchemaElement,
}

#[smart_serde_default]
#[derive(Serialize, Deserialize, SmartDefault, Clone)]
pub struct EndpointSchemaElement {
    #[smart_default(true)]
    pub frontend_facing: bool,
    pub schema: EndpointSchema,
}

impl Into<EndpointSchema> for EndpointSchemaElement {
    fn into(self) -> EndpointSchema {
        EndpointSchema {
            name: self.schema.name,
            code: self.schema.code,
            parameters: self.schema.parameters,
            returns: self.schema.returns,
            stream_response: self.schema.stream_response,
            description: self.schema.description,
            json_schema: self.schema.json_schema,
            roles: self.schema.roles,
        }
    }
}

impl FromIterator<EndpointSchemaElement> for Vec<EndpointSchema> {
    fn from_iter<T: IntoIterator<Item = EndpointSchemaElement>>(iter: T) -> Self {
        iter.into_iter().map(|element| element.schema).collect()
    }
}

impl GenElement<EndpointSchemaDefinition> for EndpointSchemaDefinition {
    fn validate_element(&self) -> eyre::Result<()> {
        Ok(())
    }
}

#[derive(Serialize, Deserialize, DefinitionVariant)]
pub struct EndpointSchemaListDefinition {
    pub service_name: String,
    pub service_id: u16,
    pub endpoints: Vec<EndpointSchemaElement>,
}

impl GenElement<EndpointSchemaListDefinition> for EndpointSchemaListDefinition {
    fn validate_element(&self) -> eyre::Result<()> {
        Ok(())
    }
}
