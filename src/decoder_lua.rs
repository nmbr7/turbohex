use mlua::{Lua, Result as LuaResult, StdLib, Value};
use std::path::PathBuf;

use crate::app::{DecoderParam, ParamType};
use crate::decode::{DecodedValue, Endian};

struct LuaDecoder {
    name: String,
    path: PathBuf,
}

pub struct LuaDecoderManager {
    lua: Lua,
    decoders: Vec<LuaDecoder>,
    loaded: bool,
}

impl LuaDecoderManager {
    pub fn new() -> Self {
        Self {
            // Sandbox: only load safe computation libraries.
            // Excludes io, os, package, debug, coroutine to prevent
            // filesystem access, command execution, and module loading.
            lua: Lua::new_with(
                StdLib::TABLE | StdLib::STRING | StdLib::UTF8 | StdLib::MATH,
                mlua::LuaOptions::default(),
            ).expect("Failed to create sandboxed Lua state"),
            decoders: Vec::new(),
            loaded: false,
        }
    }

    pub fn load_decoders(&mut self) {
        if self.loaded {
            return;
        }
        self.loaded = true;

        let config_dir = dirs_path();
        if !config_dir.exists() {
            // Create the directory so users know where to put decoders
            let _ = std::fs::create_dir_all(&config_dir);
            // Write an example decoder
            let example = config_dir.join("example.lua");
            if !example.exists() {
                let _ = std::fs::write(
                    &example,
                    r#"-- Example turbohex Lua decoder
-- This file is loaded from ~/.config/turbohex/decoders/
--
-- The decode(bytes, endian) function receives:
--   bytes  - a table of byte values (1-indexed)
--   endian - "LE" or "BE"
--
-- Return a table of {label, value} pairs.

function decode(bytes, endian)
    local results = {}

    -- Example: show byte sum
    if #bytes > 0 then
        local sum = 0
        for i = 1, #bytes do
            sum = sum + bytes[i]
        end
        table.insert(results, {label = "Byte Sum", value = tostring(sum)})
        table.insert(results, {label = "Byte Avg", value = string.format("%.1f", sum / #bytes)})
    end

    return results
end
"#,
                );
            }
            return;
        }

        // Scan for .lua files
        if let Ok(entries) = std::fs::read_dir(&config_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("lua") {
                    let name = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("unknown")
                        .to_string();
                    self.decoders.push(LuaDecoder { name, path });
                }
            }
        }
    }

    pub fn decoder_names(&self) -> Vec<String> {
        self.decoders.iter().map(|d| d.name.clone()).collect()
    }

    /// Query the optional `params()` function from a Lua decoder.
    /// Returns param definitions: [{name, type, default, choices?}]
    pub fn query_params(&self, decoder_name: &str) -> Vec<DecoderParam> {
        let decoder = match self.decoders.iter().find(|d| d.name == decoder_name) {
            Some(d) => d,
            None => return Vec::new(),
        };
        let source = match std::fs::read_to_string(&decoder.path) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        self.lua.scope(|_scope| {
            self.lua.load(&source).exec().ok();
            let params_fn: mlua::Function = match self.lua.globals().get("params") {
                Ok(f) => f,
                Err(_) => return Ok(Vec::new()),
            };
            let result: Value = params_fn.call(()).unwrap_or(Value::Nil);
            let mut params = Vec::new();
            if let Value::Table(table) = result {
                for entry in table.sequence_values::<mlua::Table>().flatten() {
                    let name: String = entry.get("name").unwrap_or_default();
                    let type_str: String = entry.get("type").unwrap_or_else(|_| "string".to_string());
                    let default: String = entry.get("default").unwrap_or_default();
                    let param_type = match type_str.as_str() {
                        "int" => ParamType::Int,
                        "bool" => ParamType::Bool,
                        "choice" => {
                            let choices: Vec<String> = entry
                                .get::<mlua::Table>("choices")
                                .ok()
                                .map(|t| {
                                    t.sequence_values::<String>()
                                        .flatten()
                                        .collect()
                                })
                                .unwrap_or_default();
                            ParamType::Choice(choices)
                        }
                        _ => ParamType::String,
                    };
                    if !name.is_empty() {
                        params.push(DecoderParam {
                            name,
                            param_type,
                            default: default.clone(),
                            value: default,
                        });
                    }
                }
            }
            Ok(params)
        }).unwrap_or_default()
    }

    pub fn decode(&mut self, bytes: &[u8], endian: Endian, enabled: &dyn Fn(&str) -> bool, params: &dyn Fn(&str) -> Vec<(String, String)>) -> Vec<DecodedValue> {
        if !self.loaded {
            self.load_decoders();
        }

        let mut results = Vec::new();

        for decoder in &self.decoders {
            if !enabled(&decoder.name) {
                continue;
            }
            let decoder_params = params(&decoder.name);
            match self.run_decoder(&decoder.path, &decoder.name, bytes, endian, &decoder_params) {
                Ok(mut vals) => results.append(&mut vals),
                Err(e) => {
                    results.push(DecodedValue {
                        label: decoder.name.clone(),
                        value: format!("error: {}", e),
                        range: None,
                        color_index: None,
                    });
                }
            }
        }

        results
    }

    fn run_decoder(
        &self,
        path: &PathBuf,
        name: &str,
        bytes: &[u8],
        endian: Endian,
        params: &[(String, String)],
    ) -> LuaResult<Vec<DecodedValue>> {
        let source = std::fs::read_to_string(path)
            .map_err(|e| mlua::Error::external(format!("Failed to read {}: {}", name, e)))?;

        self.lua.scope(|_scope| {
            self.lua.load(&source).exec()?;

            let decode_fn: mlua::Function = self.lua.globals().get("decode")?;

            // Create bytes table
            let bytes_table = self.lua.create_table()?;
            for (i, &b) in bytes.iter().enumerate() {
                bytes_table.set(i + 1, b)?; // Lua is 1-indexed
            }

            let endian_str = endian.label();

            // Build params table
            let params_table = self.lua.create_table()?;
            for (k, v) in params {
                params_table.set(k.as_str(), v.as_str())?;
            }

            let result: Value = decode_fn.call((bytes_table, endian_str, params_table))?;

            let mut decoded = Vec::new();

            if let Value::Table(table) = result {
                for pair in table.sequence_values::<mlua::Table>() {
                    if let Ok(entry) = pair {
                        let label: String = entry.get("label").unwrap_or_default();
                        let value: String = entry.get("value").unwrap_or_default();
                        let offset: Option<usize> = entry.get("offset").ok();
                        let length: Option<usize> = entry.get("length").ok();
                        let range = match (offset, length) {
                            (Some(o), Some(l)) => Some((o, l)),
                            _ => None,
                        };
                        decoded.push(DecodedValue {
                            label,
                            value,
                            range,
                            color_index: None,
                        });
                    }
                }
            }

            Ok(decoded)
        })
    }
}

fn dirs_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
        .join(".config")
        .join("turbohex")
        .join("decoders")
}
