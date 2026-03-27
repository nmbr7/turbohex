use std::path::PathBuf;
use wasmtime::*;

use crate::decode::{DecodedValue, Endian};

/// WASM decoder ABI:
///
/// The .wasm module must export:
///   memory    - linear memory
///   alloc(size: i32) -> i32   - allocate `size` bytes, return pointer
///   decode(ptr: i32, len: i32, endian: i32) -> i32
///       - ptr/len: input bytes in memory
///       - endian: 0 = LE, 1 = BE
///       - returns: pointer to JSON result string (NUL-terminated)
///
/// The JSON result format: [{"label":"...","value":"..."},...]

struct WasmDecoder {
    name: String,
    path: PathBuf,
    module: Module,
}

pub struct WasmDecoderManager {
    engine: Engine,
    decoders: Vec<WasmDecoder>,
    loaded: bool,
}

impl WasmDecoderManager {
    pub fn new() -> Self {
        let engine = Engine::default();
        Self {
            engine,
            decoders: Vec::new(),
            loaded: false,
        }
    }

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

    pub fn decode(&mut self, bytes: &[u8], endian: Endian) -> Vec<DecodedValue> {
        if !self.loaded {
            self.load_decoders();
        }

        let mut results = Vec::new();

        for i in 0..self.decoders.len() {
            let name = self.decoders[i].name.clone();
            match self.run_decoder(i, bytes, endian) {
                Ok(mut vals) => results.append(&mut vals),
                Err(e) => {
                    results.push(DecodedValue {
                        label: name,
                        value: format!("error: {}", e),
                    });
                }
            }
        }

        results
    }

    fn run_decoder(
        &self,
        idx: usize,
        bytes: &[u8],
        endian: Endian,
    ) -> anyhow::Result<Vec<DecodedValue>> {
        let decoder = &self.decoders[idx];
        let mut store = Store::new(&self.engine, ());
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
        let input_ptr = alloc_fn.call(&mut store, bytes.len() as i32)?;
        memory.data_mut(&mut store)[input_ptr as usize..input_ptr as usize + bytes.len()]
            .copy_from_slice(bytes);

        // Call decode
        let endian_val = match endian {
            Endian::Little => 0,
            Endian::Big => 1,
        };
        let result_ptr =
            decode_fn.call(&mut store, (input_ptr, bytes.len() as i32, endian_val))?;

        // Read NUL-terminated JSON string from result pointer
        let mem_data = memory.data(&store);
        let start = result_ptr as usize;
        let mut end = start;
        while end < mem_data.len() && mem_data[end] != 0 {
            end += 1;
        }
        let json_str = std::str::from_utf8(&mem_data[start..end])
            .map_err(|e| anyhow::anyhow!("{}: invalid UTF-8 in result: {}", decoder.name, e))?;

        // Parse JSON: [{"label":"...","value":"..."},...]
        parse_json_results(json_str)
    }
}

/// Minimal JSON array parser — no serde dependency needed.
/// Expects: [{"label":"...","value":"..."},...]
fn parse_json_results(json: &str) -> anyhow::Result<Vec<DecodedValue>> {
    let json = json.trim();
    if json == "[]" || json.is_empty() {
        return Ok(Vec::new());
    }

    let mut results = Vec::new();
    // Simple state machine parser for our specific JSON format
    let chars: Vec<char> = json.chars().collect();
    let len = chars.len();

    if chars.first() != Some(&'[') || chars.last() != Some(&']') {
        return Err(anyhow::anyhow!("Expected JSON array"));
    }

    let mut i = 1; // skip '['
    while i < len - 1 {
        // Skip whitespace and commas
        while i < len - 1 && (chars[i].is_whitespace() || chars[i] == ',') {
            i += 1;
        }
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

        // Parse key-value pairs inside the object
        while i < len && chars[i] != '}' {
            // Skip whitespace and commas
            while i < len && (chars[i].is_whitespace() || chars[i] == ',') {
                i += 1;
            }
            if i >= len || chars[i] == '}' {
                break;
            }

            // Parse key
            let key = parse_json_string(&chars, &mut i);
            // Skip colon and whitespace
            while i < len && (chars[i] == ':' || chars[i].is_whitespace()) {
                i += 1;
            }
            // Parse value
            let val = parse_json_string(&chars, &mut i);

            if key == "label" {
                label = val;
            } else if key == "value" {
                value = val;
            }
        }
        if i < len && chars[i] == '}' {
            i += 1;
        }

        if !label.is_empty() {
            results.push(DecodedValue { label, value });
        }
    }

    Ok(results)
}

fn parse_json_string(chars: &[char], i: &mut usize) -> String {
    let len = chars.len();
    // Skip to opening quote
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

fn decoders_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
        .join(".config")
        .join("turbohex")
        .join("decoders")
}
