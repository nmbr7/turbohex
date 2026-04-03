//! Lua decoder plugin manager.
//!
//! Loads `.lua` files from `~/.config/turbohex/decoders/` and runs their
//! `decode(bytes, endian, params)` function against the current selection.
//! Each Lua decoder runs in a sandboxed Lua state with only safe computation
//! libraries (table, string, utf8, math) — no filesystem or OS access.

use mlua::{Lua, Result as LuaResult, StdLib, Value};
use std::path::PathBuf;

use crate::app::{DecoderParam, ParamType};
use super::types::{DecodedValue, Endian};

/// A discovered Lua decoder script on disk.
struct LuaDecoder {
    /// Display name derived from the filename stem.
    name: String,
    /// Absolute path to the `.lua` file.
    path: PathBuf,
}

/// Manages discovery, loading, and execution of Lua decoder plugins.
///
/// On first use, scans `~/.config/turbohex/decoders/` for `.lua` files.
/// If the directory doesn't exist, creates it and writes an example decoder.
/// Each decode call re-reads the script from disk, enabling live editing.
pub struct LuaDecoderManager {
    /// Shared sandboxed Lua runtime (no io/os/package/debug access).
    lua: Lua,
    /// List of discovered decoder scripts.
    decoders: Vec<LuaDecoder>,
    /// Whether the initial directory scan has been performed.
    loaded: bool,
}

impl LuaDecoderManager {
    /// Creates a new manager with a sandboxed Lua state.
    ///
    /// The Lua runtime is restricted to `TABLE | STRING | UTF8 | MATH` standard
    /// libraries, preventing filesystem access, command execution, and module loading.
    pub fn new() -> Self {
        Self {
            lua: Lua::new_with(
                StdLib::TABLE | StdLib::STRING | StdLib::UTF8 | StdLib::MATH,
                mlua::LuaOptions::default(),
            )
            .expect("Failed to create sandboxed Lua state"),
            decoders: Vec::new(),
            loaded: false,
        }
    }

    /// Scans the decoder directory for `.lua` files and registers them.
    ///
    /// Creates the directory and an example decoder if it doesn't exist.
    /// This method is idempotent — subsequent calls are no-ops.
    pub fn load_decoders(&mut self) {
        if self.loaded {
            return;
        }
        self.loaded = true;

        let config_dir = decoders_path();
        if !config_dir.exists() {
            let _ = std::fs::create_dir_all(&config_dir);
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

    /// Returns the names of all discovered Lua decoders.
    pub fn decoder_names(&self) -> Vec<String> {
        self.decoders.iter().map(|d| d.name.clone()).collect()
    }

    /// Queries the optional `params()` function from a Lua decoder script.
    ///
    /// If the script exports a `params()` function, it should return a table
    /// of parameter definitions: `[{name, type, default, choices?}]`.
    /// Returns an empty vec if the function is absent or fails.
    pub fn query_params(&self, decoder_name: &str) -> Vec<DecoderParam> {
        let decoder = match self.decoders.iter().find(|d| d.name == decoder_name) {
            Some(d) => d,
            None => return Vec::new(),
        };
        let source = match std::fs::read_to_string(&decoder.path) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        self.lua
            .scope(|_scope| {
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
                        let type_str: String =
                            entry.get("type").unwrap_or_else(|_| "string".to_string());
                        let default: String = entry.get("default").unwrap_or_default();
                        let param_type = match type_str.as_str() {
                            "int" => ParamType::Int,
                            "bool" => ParamType::Bool,
                            "choice" => {
                                let choices: Vec<String> = entry
                                    .get::<mlua::Table>("choices")
                                    .ok()
                                    .map(|t| t.sequence_values::<String>().flatten().collect())
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
            })
            .unwrap_or_default()
    }

    /// Runs all enabled Lua decoders against the given bytes.
    ///
    /// # Arguments
    ///
    /// * `bytes` - The selected byte slice to decode.
    /// * `endian` - Current byte order setting.
    /// * `enabled` - Predicate that returns whether a decoder (by name) is enabled.
    /// * `params` - Returns the current parameter values for a decoder (by name).
    pub fn decode(
        &mut self,
        bytes: &[u8],
        endian: Endian,
        enabled: &dyn Fn(&str) -> bool,
        params: &dyn Fn(&str) -> Vec<(String, String)>,
    ) -> Vec<DecodedValue> {
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

    /// Executes a single Lua decoder script and parses its results.
    ///
    /// Re-reads the script from disk on each call, allowing live editing.
    /// The Lua `decode(bytes, endian, params)` function receives a 1-indexed
    /// byte table, an endian string (`"LE"` / `"BE"`), and a params table.
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

            // Create 1-indexed bytes table
            let bytes_table = self.lua.create_table()?;
            for (i, &b) in bytes.iter().enumerate() {
                bytes_table.set(i + 1, b)?;
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

/// Returns the path to the decoder plugins directory.
fn decoders_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
        .join(".config")
        .join("turbohex")
        .join("decoders")
}
