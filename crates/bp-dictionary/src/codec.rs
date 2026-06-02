use serde_json::Value;
use crate::builder::Dictionary;
use crate::error::DictError;

/// Sentinel prefix for dictionary-encoded values
const DICT_PREFIX: &str = "\x00bp:";

/// Encode a JSON value tree using dictionary substitution
pub fn encode(value: &Value, dict: &Dictionary) -> Value {
    match value {
        Value::Object(map) => {
            let mut out = serde_json::Map::new();
            for (key, val) in map {
                // Encode the key
                let encoded_key = if let Some(idx) = dict.get_idx(key) {
                    format!("{}{}", DICT_PREFIX, idx)
                } else {
                    key.clone()
                };
                // Encode the value recursively
                out.insert(encoded_key, encode(val, dict));
            }
            Value::Object(out)
        }
        Value::Array(arr) => {
            Value::Array(arr.iter().map(|v| encode(v, dict)).collect())
        }
        Value::String(s) => {
            if let Some(idx) = dict.get_idx(s) {
                Value::String(format!("{}{}", DICT_PREFIX, idx))
            } else {
                value.clone()
            }
        }
        _ => value.clone(),
    }
}

/// Decode a dictionary-encoded JSON value tree
pub fn decode(value: &Value, dict: &Dictionary) -> Result<Value, DictError> {
    match value {
        Value::Object(map) => {
            let mut out = serde_json::Map::new();
            for (key, val) in map {
                let decoded_key = decode_string(key, dict)?;
                out.insert(decoded_key, decode(val, dict)?);
            }
            Ok(Value::Object(out))
        }
        Value::Array(arr) => {
            let decoded: Result<Vec<_>, _> = arr.iter().map(|v| decode(v, dict)).collect();
            Ok(Value::Array(decoded?))
        }
        Value::String(s) => {
            Ok(Value::String(decode_string(s, dict)?))
        }
        _ => Ok(value.clone()),
    }
}

fn decode_string(s: &str, dict: &Dictionary) -> Result<String, DictError> {
    if let Some(rest) = s.strip_prefix(DICT_PREFIX) {
        let idx: u16 = rest.parse()
            .map_err(|_| DictError::IndexOutOfRange(0))?;
        dict.get_term(idx)
            .map(|t| t.to_string())
            .ok_or(DictError::IndexOutOfRange(idx))
    } else {
        Ok(s.to_string())
    }
}
