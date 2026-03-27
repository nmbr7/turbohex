/// turbohex WASM Decoder Example (Rust)
///
/// ABI contract:
///   - Export `alloc(size: i32) -> i32`  — allocate bytes in linear memory
///   - Export `decode(ptr: i32, len: i32, endian: i32) -> i32`
///       ptr/len = input bytes, endian: 0=LE 1=BE
///       returns pointer to NUL-terminated JSON string:
///       [{"label":"...","value":"..."},...]
///
/// Build:
///   rustup target add wasm32-unknown-unknown
///   cargo build --target wasm32-unknown-unknown --release
///   cp target/wasm32-unknown-unknown/release/decoder_example.wasm \
///      ~/.config/turbohex/decoders/

use std::alloc::Layout;
use std::fmt::Write;
use std::slice;

/// Allocate `size` bytes and return the pointer.
#[unsafe(no_mangle)]
pub extern "C" fn alloc(size: i32) -> i32 {
    let layout = Layout::from_size_align(size as usize, 1).unwrap();
    let ptr = unsafe { std::alloc::alloc(layout) };
    ptr as i32
}

/// Decode the given bytes. Returns pointer to NUL-terminated JSON.
#[unsafe(no_mangle)]
pub extern "C" fn decode(ptr: i32, len: i32, endian: i32) -> i32 {
    let bytes = unsafe { slice::from_raw_parts(ptr as *const u8, len as usize) };
    let _is_le = endian == 0;

    let mut json = String::from("[");
    let mut first = true;

    // --- File magic signature detection ---
    let signatures: &[(&[u8], &str)] = &[
        (b"\x7fELF", "ELF binary"),
        (b"\x89PNG\r\n\x1a\n", "PNG image"),
        (b"PK\x03\x04", "ZIP archive"),
        (b"PK\x05\x06", "ZIP (empty)"),
        (b"\xff\xd8\xff", "JPEG image"),
        (b"GIF87a", "GIF87a image"),
        (b"GIF89a", "GIF89a image"),
        (b"%PDF", "PDF document"),
        (b"MZ", "PE/DOS executable"),
        (b"\xce\xfa\xed\xfe", "Mach-O (32-bit)"),
        (b"\xcf\xfa\xed\xfe", "Mach-O (64-bit)"),
        (b"\xfe\xed\xfa\xce", "Mach-O (32 BE)"),
        (b"\xfe\xed\xfa\xcf", "Mach-O (64 BE)"),
        (b"\xca\xfe\xba\xbe", "Mach-O Universal"),
        (b"RIFF", "RIFF container"),
        (b"\x1f\x8b", "gzip compressed"),
        (b"BZh", "bzip2 compressed"),
        (b"\xfd7zXZ\x00", "xz compressed"),
        (b"\x00asm", "WebAssembly"),
        (b"SQLite format 3", "SQLite database"),
        (b"\xd0\xcf\x11\xe0", "MS Office (OLE)"),
    ];

    for (magic, desc) in signatures {
        if bytes.len() >= magic.len() && &bytes[..magic.len()] == *magic {
            append_entry(&mut json, &mut first, "Magic", desc);
            break;
        }
    }

    // --- Entropy estimate (useful for detecting compressed/encrypted data) ---
    if bytes.len() >= 16 {
        let mut counts = [0u32; 256];
        for &b in bytes {
            counts[b as usize] += 1;
        }
        let len_f = bytes.len() as f64;
        let mut entropy = 0.0f64;
        for &c in &counts {
            if c > 0 {
                let p = c as f64 / len_f;
                entropy -= p * p.log2();
            }
        }
        let label = if entropy > 7.5 {
            "high (compressed/encrypted?)"
        } else if entropy > 5.0 {
            "medium"
        } else if entropy > 3.0 {
            "low"
        } else {
            "very low (sparse/uniform)"
        };
        let mut buf = String::new();
        let whole = entropy as u64;
        let frac = ((entropy - whole as f64) * 100.0) as u64;
        write!(buf, "{}.{:02} bits/byte ({})", whole, frac, label).ok();
        append_entry(&mut json, &mut first, "Entropy", &buf);
    }

    // --- Null byte ratio ---
    if !bytes.is_empty() {
        let null_count = bytes.iter().filter(|&&b| b == 0).count();
        let pct = (null_count as f64 / bytes.len() as f64 * 100.0) as u32;
        let mut buf = String::new();
        write!(buf, "{}/{} ({}%)", null_count, bytes.len(), pct).ok();
        append_entry(&mut json, &mut first, "Null bytes", &buf);
    }

    // --- Printable ASCII ratio ---
    if !bytes.is_empty() {
        let printable = bytes
            .iter()
            .filter(|&&b| b >= 0x20 && b <= 0x7e)
            .count();
        let pct = (printable as f64 / bytes.len() as f64 * 100.0) as u32;
        let mut buf = String::new();
        write!(buf, "{}/{} ({}%)", printable, bytes.len(), pct).ok();
        append_entry(&mut json, &mut first, "Printable", &buf);
    }

    json.push(']');
    json.push('\0');

    // Leak the string so WASM memory keeps it alive, return its pointer
    let bytes = json.into_bytes();
    let ptr = bytes.as_ptr() as i32;
    std::mem::forget(bytes);
    ptr
}

fn append_entry(json: &mut String, first: &mut bool, label: &str, value: &str) {
    if !*first {
        json.push(',');
    }
    *first = false;
    json.push_str(r#"{"label":""#);
    json_escape_into(json, label);
    json.push_str(r#"","value":""#);
    json_escape_into(json, value);
    json.push_str(r#""}"#);
}

fn json_escape_into(out: &mut String, s: &str) {
    for c in s.chars() {
        match c {
            '"' => out.push_str(r#"\""#),
            '\\' => out.push_str(r#"\\"#),
            '\n' => out.push_str(r#"\n"#),
            '\t' => out.push_str(r#"\t"#),
            _ => out.push(c),
        }
    }
}
