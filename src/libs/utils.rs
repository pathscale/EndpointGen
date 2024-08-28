use convert_case::{Case, Casing};
use eyre::*;
use serde::Serialize;
use std::fmt::Write;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn get_log_id() -> u64 {
    chrono::Utc::now().timestamp_micros() as _
}

pub fn get_conn_id() -> u32 {
    chrono::Utc::now().timestamp_micros() as _
}

pub fn encode_header<T: Serialize>(v: T, schema: EndpointSchema) -> Result<String> {
    let mut s = String::new();
    write!(s, "0{}", schema.name.to_ascii_lowercase())?;
    let v = serde_json::to_value(&v)?;

    for (i, f) in schema.parameters.iter().enumerate() {
        let key = f.name.to_case(Case::Camel);
        let value = v.get(&key).with_context(|| format!("key: {}", key))?;
        if value.is_null() {
            continue;
        }
        write!(
            s,
            ", {}{}",
            i + 1,
            urlencoding::encode(&value.to_string().replace("\"", ""))
        )?;
    }
    Ok(s)
}

pub fn get_time_seconds() -> u32 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as _
}

pub fn get_time_milliseconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as _
}
pub fn hex_decode(s: &[u8]) -> Result<Vec<u8>> {
    if s.starts_with(b"0x") {
        Ok(hex::decode(&s[2..])?)
    } else {
        Ok(hex::decode(s)?)
    }
}
