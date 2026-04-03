//! WASM decoder plugin manager.
//!
//! Loads `.wasm` files from `~/.config/turbohex/decoders/` and executes their
//! `decode` export against the current selection. Each WASM module runs in an
//! isolated wasmtime instance with fuel limits to prevent infinite loops.
//!
//! # WASM Decoder ABI
//!
//! The `.wasm` module must export:
//! - `memory` — linear memory
//! - `alloc(size: i32) -> i32` — allocate `size` bytes, return pointer
//! - `decode(ptr: i32, len: i32, endian: i32) -> i32` — decode input bytes
//!   - `endian`: 0 = LE, 1 = BE
//!   - returns: pointer to NUL-terminated JSON result string
//!
//! Optional exports:
//! - `params() -> i32` — return pointer to NUL-terminated JSON parameter definitions
//! - `decode_with_params(ptr, len, endian, params_ptr, params_len) -> i32` — decode with parameters

pub mod json;

use std::path::PathBuf;
use wasmtime::*;

use crate::app::{DecoderParam};
use super::types::{DecodedValue, Endian};
use json::{build_params_json, parse_json_results, parse_param_definitions};

/// A compiled WASM decoder module ready for instantiation.
struct WasmDecoder {
    /// Display name derived from the filename stem.
    name: String,
    /// Absolute path to the `.wasm` file.
    #[allow(dead_code)]
    path: PathBuf,
    /// Pre-compiled WASM module (compiled once at load time).
    module: Module,
}

/// Manages discovery, compilation, and execution of WASM decoder plugins.
///
/// WASM modules are compiled once during [`load_decoders`](Self::load_decoders)
/// and instantiated fresh for each decode call to ensure isolation.
/// A fuel limit (10M instructions) prevents runaway modules.
pub struct WasmDecoderManager {
    /// Shared wasmtime engine with fuel metering enabled.
    engine: Engine,
    /// List of compiled decoder modules.
    decoders: Vec<WasmDecoder>,
    /// Whether the initial directory scan has been performed.
    loaded: bool,
}

impl WasmDecoderManager {
    /// Creates a new manager with a fuel-metered wasmtime engine.
    pub fn new() -> Self {
        let mut config = Config::new();
        config.consume_fuel(true);
        let engine = Engine::new(&config).expect("Failed to create WASM engine");
        Self {
            engine,
            decoders: Vec::new(),
            loaded: false,
        }
    }

    /// Scans `~/.config/turbohex/decoders/` for `.wasm` files and compiles them.
    ///
    /// Creates the directory if it doesn't exist. Compilation errors are printed
    /// to stderr but don't prevent other decoders from loading.
    /// This method is idempotent — subsequent calls are no-ops.
    pub fn load_decoders(&mut self) {
        if self.loaded {
            return;
        }
        self.loaded = true;

        let config_dir = decoders_path();
        if !config_dir.exists() {
            let _ = std::fs::create_dir_all(&config_dir);
            return;
        }

        if let Ok(entries) = std::fs::read_dir(&config_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("wasm") {
                    let name = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("unknown")
                        .to_string();

                    match Module::from_file(&self.engine, &path) {
                        Ok(module) => {
                            self.decoders.push(WasmDecoder { name, path, module });
                        }
                        Err(e) => {
                            eprintln!("Failed to load WASM decoder {}: {}", path.display(), e);
                        }
                    }
                }
            }
        }
    }

    /// Returns the names of all discovered WASM decoders.
    pub fn decoder_names(&self) -> Vec<String> {
        self.decoders.iter().map(|d| d.name.clone()).collect()
    }

    /// Queries the optional `params()` export from a WASM decoder.
    ///
    /// Instantiates the module, calls `params()`, and parses the returned
    /// NUL-terminated JSON string into parameter definitions.
    /// Returns an empty vec if the export is absent or the call fails.
    pub fn query_params(&self, decoder_name: &str) -> Vec<DecoderParam> {
        let decoder = match self.decoders.iter().find(|d| d.name == decoder_name) {
            Some(d) => d,
            None => return Vec::new(),
        };
        let mut store = Store::new(&self.engine, ());
        store.set_fuel(1_000_000).ok();
        let instance = match Instance::new(&mut store, &decoder.module, &[]) {
            Ok(i) => i,
            Err(_) => return Vec::new(),
        };
        let memory = match instance.get_memory(&mut store, "memory") {
            Some(m) => m,
            None => return Vec::new(),
        };
        let params_fn = match instance.get_typed_func::<(), i32>(&mut store, "params") {
            Ok(f) => f,
            Err(_) => return Vec::new(),
        };
        let result_ptr = match params_fn.call(&mut store, ()) {
            Ok(p) => p,
            Err(_) => return Vec::new(),
        };
        if result_ptr < 0 {
            return Vec::new();
        }
        let mem_data = memory.data(&store);
        let start = result_ptr as usize;
        if start >= mem_data.len() {
            return Vec::new();
        }
        let json_str = match read_nul_terminated_str(mem_data, start) {
            Some(s) => s,
            None => return Vec::new(),
        };
        parse_param_definitions(json_str)
    }

    /// Runs all enabled WASM decoders against the given bytes.
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

        for i in 0..self.decoders.len() {
            let name = self.decoders[i].name.clone();
            if !enabled(&name) {
                continue;
            }
            let decoder_params = params(&name);
            match self.run_decoder(i, bytes, endian, &decoder_params) {
                Ok(mut vals) => results.append(&mut vals),
                Err(e) => {
                    results.push(DecodedValue {
                        label: name,
                        value: format!("error: {}", e),
                        range: None,
                        color_index: None,
                    });
                }
            }
        }

        results
    }

    /// Instantiates and runs a single WASM decoder module.
    ///
    /// Creates a fresh wasmtime store with a 10M instruction fuel limit,
    /// copies the input bytes into the module's linear memory via `alloc`,
    /// and calls either `decode_with_params` or `decode` depending on
    /// whether parameters are provided and the export exists.
    fn run_decoder(
        &self,
        idx: usize,
        bytes: &[u8],
        endian: Endian,
        params: &[(String, String)],
    ) -> anyhow::Result<Vec<DecodedValue>> {
        let decoder = &self.decoders[idx];
        let mut store = Store::new(&self.engine, ());
        store.set_fuel(10_000_000)?;
        let instance = Instance::new(&mut store, &decoder.module, &[])?;

        let memory = instance
            .get_memory(&mut store, "memory")
            .ok_or_else(|| anyhow::anyhow!("{}: no 'memory' export", decoder.name))?;

        let alloc_fn = instance
            .get_typed_func::<i32, i32>(&mut store, "alloc")
            .map_err(|e| anyhow::anyhow!("{}: missing 'alloc' export: {}", decoder.name, e))?;

        let decode_fn = instance
            .get_typed_func::<(i32, i32, i32), i32>(&mut store, "decode")
            .map_err(|e| anyhow::anyhow!("{}: missing 'decode' export: {}", decoder.name, e))?;

        // Allocate space for input bytes and copy them in
        let input_len = i32::try_from(bytes.len())
            .map_err(|_| anyhow::anyhow!("{}: input too large for WASM i32", decoder.name))?;
        let input_ptr = alloc_fn.call(&mut store, input_len)?;
        if input_ptr < 0 {
            return Err(anyhow::anyhow!(
                "{}: alloc returned negative pointer",
                decoder.name
            ));
        }
        let ptr = input_ptr as usize;
        let mem_len = memory.data(&store).len();
        if ptr.saturating_add(bytes.len()) > mem_len {
            return Err(anyhow::anyhow!(
                "{}: alloc returned out-of-bounds pointer",
                decoder.name
            ));
        }
        memory.data_mut(&mut store)[ptr..ptr + bytes.len()].copy_from_slice(bytes);

        let endian_val = match endian {
            Endian::Little => 0,
            Endian::Big => 1,
        };

        // Try decode_with_params first, fall back to decode
        let result_ptr = if !params.is_empty() {
            if let Ok(decode_wp) = instance
                .get_typed_func::<(i32, i32, i32, i32, i32), i32>(&mut store, "decode_with_params")
            {
                let params_json = build_params_json(params);
                let params_len = i32::try_from(params_json.len()).map_err(|_| {
                    anyhow::anyhow!("{}: params too large for WASM i32", decoder.name)
                })?;
                let params_ptr = alloc_fn.call(&mut store, params_len)?;
                if params_ptr < 0 {
                    return Err(anyhow::anyhow!(
                        "{}: alloc returned negative pointer for params",
                        decoder.name
                    ));
                }
                let pp = params_ptr as usize;
                let ml = memory.data(&store).len();
                if pp.saturating_add(params_json.len()) > ml {
                    return Err(anyhow::anyhow!(
                        "{}: alloc returned out-of-bounds pointer for params",
                        decoder.name
                    ));
                }
                memory.data_mut(&mut store)[pp..pp + params_json.len()]
                    .copy_from_slice(params_json.as_bytes());
                decode_wp.call(
                    &mut store,
                    (input_ptr, input_len, endian_val, params_ptr, params_len),
                )?
            } else {
                decode_fn.call(&mut store, (input_ptr, input_len, endian_val))?
            }
        } else {
            decode_fn.call(&mut store, (input_ptr, input_len, endian_val))?
        };

        // Read NUL-terminated JSON string from result pointer
        if result_ptr < 0 {
            return Err(anyhow::anyhow!(
                "{}: decode returned negative pointer",
                decoder.name
            ));
        }
        let mem_data = memory.data(&store);
        let start = result_ptr as usize;
        if start >= mem_data.len() {
            return Err(anyhow::anyhow!(
                "{}: decode returned out-of-bounds pointer",
                decoder.name
            ));
        }
        let json_str = read_nul_terminated_str(mem_data, start).ok_or_else(|| {
            anyhow::anyhow!("{}: invalid UTF-8 in result", decoder.name)
        })?;

        parse_json_results(json_str)
    }
}

/// Reads a NUL-terminated UTF-8 string from a byte slice starting at `start`.
///
/// Scans up to 1MB from the start position to find the NUL terminator.
/// Returns `None` if the string is not valid UTF-8.
fn read_nul_terminated_str(mem_data: &[u8], start: usize) -> Option<&str> {
    let scan_limit = mem_data.len().min(start + 1024 * 1024);
    let mut end = start;
    while end < scan_limit && mem_data[end] != 0 {
        end += 1;
    }
    std::str::from_utf8(&mem_data[start..end]).ok()
}

/// Returns the path to the decoder plugins directory.
fn decoders_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
        .join(".config")
        .join("turbohex")
        .join("decoders")
}
