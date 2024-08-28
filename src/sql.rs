use crate::model::{Field, ProceduralFunction, Type};
use crate::Data;
use convert_case::{Case, Casing};
use itertools::Itertools;
use std::fs::File;
use std::io::Write;

pub const PARAM_PREFIX: &str = "a_";

pub trait ToSql {
    fn to_sql(&self) -> String;
}

impl ToSql for Type {
    fn to_sql(&self) -> String {
        match self {
            Type::Date => "int".to_owned(), // TODO: fix things
            Type::Int => "int".to_owned(),
            Type::BigInt => "bigint".to_owned(),
            Type::TimeStampMs => "bigint".to_owned(),
            Type::Numeric => "double precision".to_owned(),
            Type::Struct { fields, .. } => {
                let fields = fields
                    .iter()
                    .map(|x| format!("\"{}\" {}", x.name, x.ty.to_sql()));
                format!(
                    "table (\n{}\n)",
                    fields.map(|x| format!("    {}", x)).join(",\n")
                )
            }
            Type::StructRef(_name) => "jsonb".to_owned(),
            Type::Object => "jsonb".to_owned(),
            Type::DataTable { .. } => {
                todo!()
            }
            Type::Vec(fields) => {
                format!("{}[]", fields.to_sql())
            }
            Type::Unit => "void".to_owned(),
            Type::Optional(t) => t.to_sql().to_string(),
            Type::Boolean => "boolean".to_owned(),
            Type::String => "varchar".to_owned(),
            Type::Bytea => "bytea".to_owned(),
            Type::UUID => "uuid".to_owned(),
            Type::Inet => "inet".to_owned(),
            Type::Enum { name, .. } => format!("enum_{}", name),
            Type::EnumRef(name) => format!("enum_{}", name),
            // 38 digits in total, with 4-18 decimal digits. So to be exact we need 38+18 digits
            Type::BlockchainDecimal => "decimal(56, 18)".to_owned(),
            Type::BlockchainAddress => "varchar".to_owned(),
            Type::BlockchainTransactionHash => "varchar".to_owned(),
        }
    }
}
impl ToSql for ProceduralFunction {
    fn to_sql(&self) -> String {
        let params = self
            .parameters
            .iter()
            .map(|x| match &x.ty {
                Type::Optional(y) => {
                    format!("{}{} {} DEFAULT NULL", PARAM_PREFIX, x.name, y.to_sql())
                }
                y => format!("{}{} {}", PARAM_PREFIX, x.name, y.to_sql()),
            })
            .join(", ");
        format!(
            "
CREATE OR REPLACE FUNCTION api.{name}({params})
RETURNS {returns}
LANGUAGE plpgsql
AS $$
    {body}
$$;
        ",
            name = self.name,
            params = params,
            returns = match &self.return_row_type {
                Type::Struct { fields, .. } if fields.is_empty() => "void".to_owned(),
                x => x.to_sql(),
            },
            body = self.body
        )
    }
}

pub fn gen_db_sql(data: &Data) -> eyre::Result<()> {
    let funcs = &data.pg_funcs;

    let db_filename = data.project_root.join("db/api.sql");
    let mut f = File::create(db_filename)?;
    writeln!(&mut f, "CREATE SCHEMA IF NOT EXISTS api;")?;
    for func in funcs {
        writeln!(&mut f, "{}", func.to_sql())?;
    }
    for srv in &data.services {
        writeln!(
            &mut f,
            "{}",
            ProceduralFunction::new(
                format!("{}_SERVICE", srv.name.to_case(Case::ScreamingSnake)),
                vec![],
                vec![Field::new("code", Type::Int)],
                format!("BEGIN RETURN QUERY SELECT {}; END", srv.id),
            )
            .to_sql()
        )?;
    }
    f.flush()?;
    drop(f);

    Ok(())
}

pub fn gen_model_sql(data: &Data) -> eyre::Result<()> {
    let db_filename = data.project_root.join("db").join("model.sql");

    // Ensure the parent directories exist
    if let Some(parent) = db_filename.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut f = File::create(db_filename)?;

    for e in &data.enums {
        match e {
            Type::Enum { name, variants } => {
                writeln!(
                    &mut f,
                    "CREATE TYPE enum_{} AS ENUM ({});",
                    name,
                    variants.iter().map(|x| format!("'{}'", x.name)).join(", ")
                )?;
            }
            _ => unreachable!(),
        }
    }
    f.flush()?;
    drop(f);
    Ok(())
}
