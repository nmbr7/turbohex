//! Minimal JSON parsing for WASM decoder communication.
//!
//! Provides hand-rolled parsers for the JSON formats used by the WASM decoder ABI,
//! avoiding a serde dependency. Handles the decode result array format and the
//! parameter definition array format.

use wasmtime::anyhow;

use crate::app::{DecoderParam, ParamType};
use super::super::types::DecodedValue;

/// Parses a JSON array of decode results from a WASM decoder.
///
/// Expected format: `[{"label":"...","value":"...","offset":N,"length":N},...]`
///
/// The `offset` and `length` fields are optional numeric fields for range mapping.
/// Returns an error if the input is not a valid JSON array.
pub fn parse_json_results(json: &str) -> anyhow::Result<Vec<DecodedValue>> {
    let json = json.trim();
    if json == "[]" || json.is_empty() {
        return Ok(Vec::new());
    }

    let mut results = Vec::new();
    let chars: Vec<char> = json.chars().collect();
    let len = chars.len();

    if chars.first() != Some(&'[') || chars.last() != Some(&']') {
        return Err(anyhow::anyhow!("Expected JSON array"));
    }

    let mut i = 1; // skip '['
    while i < len - 1 {
        skip_ws_comma(&chars, &mut i, len);
        if i >= len - 1 || chars[i] == ']' {
            break;
        }
        if chars[i] != '{' {
            i += 1;
            continue;
        }
        i += 1; // skip '{'

        let mut label = String::new();
        let mut value = String::new();
        let mut offset: Option<usize> = None;
        let mut length: Option<usize> = None;

        while i < len && chars[i] != '}' {
            skip_ws_comma(&chars, &mut i, len);
            if i >= len || chars[i] == '}' {
                break;
            }

            let key = parse_json_string(&chars, &mut i);
            // Skip colon and whitespace
            while i < len && (chars[i] == ':' || chars[i].is_whitespace()) {
                i += 1;
            }
            // Determine if value is string or number
            if i < len && chars[i] == '"' {
                let val = parse_json_string(&chars, &mut i);
                match key.as_str() {
                    "label" => label = val,
                    "value" => value = val,
                    _ => {}
                }
            } else if i < len && (chars[i].is_ascii_digit() || chars[i] == '-') {
                let num = parse_json_number(&chars, &mut i);
                match key.as_str() {
                    "offset" => offset = Some(num as usize),
                    "length" => length = Some(num as usize),
                    _ => {}
                }
            } else {
                // skip unknown value types
                while i < len && chars[i] != ',' && chars[i] != '}' {
                    i += 1;
                }
            }
        }
        if i < len && chars[i] == '}' {
            i += 1;
        }

        if !label.is_empty() {
            let range = match (offset, length) {
                (Some(o), Some(l)) => Some((o, l)),
                _ => None,
            };
            results.push(DecodedValue {
                label,
                value,
                range,
                color_index: None,
            });
        }
    }

    Ok(results)
}

/// Parses a JSON array of parameter definitions from a WASM decoder's `params()` export.
///
/// Expected format: `[{"name":"...","type":"...","default":"...","choices":["..."]}]`
///
/// Returns an empty vec if the input is empty or malformed.
pub fn parse_param_definitions(json: &str) -> Vec<DecoderParam> {
    let json = json.trim();
    if json == "[]" || json.is_empty() {
        return Vec::new();
    }
    let chars: Vec<char> = json.chars().collect();
    let len = chars.len();
    if chars.first() != Some(&'[') || chars.last() != Some(&']') {
        return Vec::new();
    }

    let mut params = Vec::new();
    let mut i = 1;
    while i < len - 1 {
        skip_ws_comma(&chars, &mut i, len);
        if i >= len - 1 || chars[i] == ']' {
            break;
        }
        if chars[i] != '{' {
            i += 1;
            continue;
        }
        i += 1;

        let mut name = String::new();
        let mut type_str = String::from("string");
        let mut default = String::new();
        let mut choices: Vec<String> = Vec::new();

        while i < len && chars[i] != '}' {
            skip_ws_comma(&chars, &mut i, len);
            if i >= len || chars[i] == '}' {
                break;
            }
            let key = parse_json_string(&chars, &mut i);
            while i < len && (chars[i] == ':' || chars[i].is_whitespace()) {
                i += 1;
            }
            if i < len && chars[i] == '"' {
                let val = parse_json_string(&chars, &mut i);
                match key.as_str() {
                    "name" => name = val,
                    "type" => type_str = val,
                    "default" => default = val,
                    _ => {}
                }
            } else if i < len && chars[i] == '[' {
                // Parse choices array
                i += 1;
                while i < len && chars[i] != ']' {
                    skip_ws_comma(&chars, &mut i, len);
                    if i < len && chars[i] == '"' {
                        choices.push(parse_json_string(&chars, &mut i));
                    } else if i < len && chars[i] != ']' {
                        i += 1;
                    }
                }
                if i < len {
                    i += 1; // skip ']'
                }
            } else {
                while i < len && chars[i] != ',' && chars[i] != '}' {
                    i += 1;
                }
            }
        }
        if i < len && chars[i] == '}' {
            i += 1;
        }

        if !name.is_empty() {
            let param_type = match type_str.as_str() {
                "int" => ParamType::Int,
                "bool" => ParamType::Bool,
                "choice" => ParamType::Choice(choices),
                _ => ParamType::String,
            };
            params.push(DecoderParam {
                name,
                param_type,
                default: default.clone(),
                value: default,
            });
        }
    }
    params
}

/// Builds a JSON object string from parameter key-value pairs: `{"key":"value",...}`.
///
/// Used to pass parameter values to a WASM decoder's `decode_with_params` export.
pub fn build_params_json(params: &[(String, String)]) -> String {
    let mut json = String::from("{");
    for (i, (k, v)) in params.iter().enumerate() {
        if i > 0 {
            json.push(',');
        }
        json.push('"');
        json_escape_into(&mut json, k);
        json.push_str("\":\"");
        json_escape_into(&mut json, v);
        json.push('"');
    }
    json.push('}');
    json
}

/// Skips whitespace and commas in a character slice, advancing the index.
fn skip_ws_comma(chars: &[char], i: &mut usize, len: usize) {
    while *i < len && (chars[*i].is_whitespace() || chars[*i] == ',') {
        *i += 1;
    }
}

/// Parses a JSON number (integer) from a character slice, advancing the index.
fn parse_json_number(chars: &[char], i: &mut usize) -> i64 {
    let len = chars.len();
    let start = *i;
    if *i < len && chars[*i] == '-' {
        *i += 1;
    }
    while *i < len && chars[*i].is_ascii_digit() {
        *i += 1;
    }
    let s: String = chars[start..*i].iter().collect();
    s.parse().unwrap_or(0)
}

/// Parses a JSON string (handling escape sequences) from a character slice, advancing the index.
fn parse_json_string(chars: &[char], i: &mut usize) -> String {
    let len = chars.len();
    while *i < len && chars[*i] != '"' {
        *i += 1;
    }
    if *i >= len {
        return String::new();
    }
    *i += 1; // skip opening '"'

    let mut s = String::new();
    while *i < len && chars[*i] != '"' {
        if chars[*i] == '\\' && *i + 1 < len {
            *i += 1;
            match chars[*i] {
                'n' => s.push('\n'),
                't' => s.push('\t'),
                '\\' => s.push('\\'),
                '"' => s.push('"'),
                '/' => s.push('/'),
                _ => {
                    s.push('\\');
                    s.push(chars[*i]);
                }
            }
        } else {
            s.push(chars[*i]);
        }
        *i += 1;
    }
    if *i < len {
        *i += 1; // skip closing '"'
    }
    s
}

/// Appends a JSON-escaped version of `s` to `out`.
fn json_escape_into(out: &mut String, s: &str) {
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
}
