use serde_json::Value;
use std::sync::OnceLock;

static CFG: OnceLock<Value> = OnceLock::new();

fn path() -> std::path::PathBuf {
    if let Ok(p) = std::env::var("BAPI_CONFIG") {
        return p.into();
    }
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../config.json")
}

pub fn all() -> Value {
    CFG.get_or_init(|| {
        std::fs::read(path())
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or(Value::Null)
    })
    .clone()
}

pub fn get(key: &str) -> String {
    all()
        .get(key)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}
