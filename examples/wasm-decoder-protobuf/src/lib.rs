/// turbohex WASM Decoder — Protobuf Wire Format Parser
///
/// Schema-less protobuf decoder using `prost` encoding primitives.
/// Decodes raw protobuf wire format fields showing field numbers,
/// wire types, and interpreted values with byte-range highlighting.
///
/// ABI contract:
///   - Export `alloc(size: i32) -> i32`
///   - Export `decode(ptr: i32, len: i32, endian: i32) -> i32`
///       returns NUL-terminated JSON:
///       [{"label":"...","value":"...","offset":N,"length":N},...]
///
/// Build:
///   rustup target add wasm32-unknown-unknown
///   cargo build --target wasm32-unknown-unknown --release
///   cp target/wasm32-unknown-unknown/release/decoder_protobuf.wasm \
///      ~/.config/turbohex/decoders/

use prost::encoding::{decode_key, decode_varint, WireType};
use std::alloc::Layout;
use std::slice;

const MAX_DEPTH: usize = 3;

#[unsafe(no_mangle)]
pub extern "C" fn alloc(size: i32) -> i32 {
    let layout = Layout::from_size_align(size as usize, 1).unwrap();
    let ptr = unsafe { std::alloc::alloc(layout) };
    ptr as i32
}

#[unsafe(no_mangle)]
pub extern "C" fn decode(ptr: i32, len: i32, endian: i32) -> i32 {
    let bytes = unsafe { slice::from_raw_parts(ptr as *const u8, len as usize) };
    let is_le = endian == 0;

    let mut out = JsonBuilder::new();

    if bytes.is_empty() {
        return out.finish();
    }

    decode_fields(&mut out, bytes, 0, 0, is_le);

    out.finish()
}

fn decode_fields(out: &mut JsonBuilder, data: &[u8], base_offset: usize, depth: usize, is_le: bool) {
    let mut buf: &[u8] = data;
    let mut pos = 0usize;

    while !buf.is_empty() {
        let field_start = pos;

        // Decode field key (tag + wire type)
        let (tag, wire_type) = match decode_key(&mut buf) {
            Ok(kv) => kv,
            Err(_) => {
                if pos > 0 {
                    out.entry("", &fmt_error("Truncated", base_offset + pos));
                }
                return;
            }
        };
        let key_len = (data.len() - pos) - buf.len();
        pos += key_len;

        // Sanity check: field number 0 is invalid
        if tag == 0 {
            out.entry("", &fmt_error("Invalid field 0", base_offset + field_start));
            return;
        }

        let indent = "  ".repeat(depth);

        match wire_type {
            WireType::Varint => {
                let val = match decode_varint(&mut buf) {
                    Ok(v) => v,
                    Err(_) => {
                        out.entry(
                            &format!("{}Field {} (varint)", indent, tag),
                            &fmt_error("Truncated varint", base_offset + pos),
                        );
                        return;
                    }
                };
                let val_len = (data.len() - pos) - buf.len();
                let total_len = key_len + val_len;
                pos += val_len;

                let signed = zigzag_decode(val);
                let value = if val <= 1 {
                    format!("{} / signed: {} / bool: {}", val, signed, val == 1)
                } else {
                    format!("{} / signed: {}", val, signed)
                };
                out.entry_range(
                    &format!("{}Field {} (varint)", indent, tag),
                    &value,
                    base_offset + field_start,
                    total_len,
                );
            }

            WireType::SixtyFourBit => {
                if buf.len() < 8 {
                    out.entry(
                        &format!("{}Field {} (fixed64)", indent, tag),
                        &fmt_error("Truncated", base_offset + pos),
                    );
                    return;
                }
                let fixed_bytes: [u8; 8] = buf[..8].try_into().unwrap();
                buf = &buf[8..];
                let total_len = key_len + 8;

                let (u_val, i_val, f_val) = if is_le {
                    (
                        u64::from_le_bytes(fixed_bytes),
                        i64::from_le_bytes(fixed_bytes),
                        f64::from_le_bytes(fixed_bytes),
                    )
                } else {
                    (
                        u64::from_be_bytes(fixed_bytes),
                        i64::from_be_bytes(fixed_bytes),
                        f64::from_be_bytes(fixed_bytes),
                    )
                };

                let value = if f_val.is_finite() && f_val != 0.0 {
                    format!("{} / {} / {:.6}", u_val, i_val, f_val)
                } else {
                    format!("{} / {}", u_val, i_val)
                };
                out.entry_range(
                    &format!("{}Field {} (fixed64)", indent, tag),
                    &value,
                    base_offset + field_start,
                    total_len,
                );
                pos += 8;
            }

            WireType::LengthDelimited => {
                let length_start = pos;
                let length = match decode_varint(&mut buf) {
                    Ok(v) => v as usize,
                    Err(_) => {
                        out.entry(
                            &format!("{}Field {} (bytes)", indent, tag),
                            &fmt_error("Truncated length", base_offset + pos),
                        );
                        return;
                    }
                };
                let length_varint_len = (data.len() - length_start) - buf.len();
                pos = length_start + length_varint_len;

                if buf.len() < length {
                    out.entry(
                        &format!("{}Field {} (bytes)", indent, tag),
                        &fmt_error("Truncated data", base_offset + pos),
                    );
                    return;
                }

                let field_data = &buf[..length];
                buf = &buf[length..];
                let total_len = key_len + length_varint_len + length;

                // Try to interpret the contents
                if length == 0 {
                    out.entry_range(
                        &format!("{}Field {} (bytes)", indent, tag),
                        "<empty>",
                        base_offset + field_start,
                        total_len,
                    );
                } else if depth < MAX_DEPTH && looks_like_protobuf(field_data) {
                    // Nested protobuf message
                    let data_offset = base_offset + pos;
                    out.entry_range(
                        &format!("{}Field {} (message)", indent, tag),
                        &format!("{} bytes", length),
                        base_offset + field_start,
                        total_len,
                    );
                    decode_fields(
                        out,
                        field_data,
                        data_offset,
                        depth + 1,
                        is_le,
                    );
                } else if let Ok(s) = core::str::from_utf8(field_data) {
                    if is_printable(s) {
                        let preview = if s.len() > 60 {
                            format!("{}...", &s[..57])
                        } else {
                            s.to_string()
                        };
                        out.entry_range(
                            &format!("{}Field {} (string)", indent, tag),
                            &preview,
                            base_offset + field_start,
                            total_len,
                        );
                    } else {
                        out.entry_range(
                            &format!("{}Field {} (bytes)", indent, tag),
                            &hex_preview(field_data, 30),
                            base_offset + field_start,
                            total_len,
                        );
                    }
                } else {
                    out.entry_range(
                        &format!("{}Field {} (bytes)", indent, tag),
                        &hex_preview(field_data, 30),
                        base_offset + field_start,
                        total_len,
                    );
                }
                pos += length;
            }

            WireType::ThirtyTwoBit => {
                if buf.len() < 4 {
                    out.entry(
                        &format!("{}Field {} (fixed32)", indent, tag),
                        &fmt_error("Truncated", base_offset + pos),
                    );
                    return;
                }
                let fixed_bytes: [u8; 4] = buf[..4].try_into().unwrap();
                buf = &buf[4..];
                let total_len = key_len + 4;

                let (u_val, i_val, f_val) = if is_le {
                    (
                        u32::from_le_bytes(fixed_bytes),
                        i32::from_le_bytes(fixed_bytes),
                        f32::from_le_bytes(fixed_bytes),
                    )
                } else {
                    (
                        u32::from_be_bytes(fixed_bytes),
                        i32::from_be_bytes(fixed_bytes),
                        f32::from_be_bytes(fixed_bytes),
                    )
                };

                let value = if f_val.is_finite() && f_val != 0.0 {
                    format!("{} / {} / {:.6}", u_val, i_val, f_val)
                } else {
                    format!("{} / {}", u_val, i_val)
                };
                out.entry_range(
                    &format!("{}Field {} (fixed32)", indent, tag),
                    &value,
                    base_offset + field_start,
                    total_len,
                );
                pos += 4;
            }

            WireType::StartGroup | WireType::EndGroup => {
                out.entry_range(
                    &format!("{}Field {} (group)", indent, tag),
                    "deprecated wire type",
                    base_offset + field_start,
                    key_len,
                );
            }
        }
    }
}

/// Heuristic: check if bytes look like a valid protobuf message.
/// Tries to decode the first few fields and checks that tags and wire types are sensible.
fn looks_like_protobuf(data: &[u8]) -> bool {
    if data.is_empty() {
        return false;
    }

    let mut buf: &[u8] = data;
    let mut field_count = 0;
    let mut prev_tag = 0u32;

    while !buf.is_empty() && field_count < 10 {
        let (tag, wire_type) = match decode_key(&mut buf) {
            Ok(kv) => kv,
            Err(_) => return false,
        };

        // Field number must be positive and reasonable
        if tag == 0 || tag > 536_870_911 {
            return false;
        }

        // Field numbers should generally be ascending (not required, but a good heuristic)
        // Allow repeated fields (same tag)
        if field_count > 0 && tag < prev_tag && prev_tag - tag > 100 {
            return false;
        }

        match wire_type {
            WireType::Varint => {
                if decode_varint(&mut buf).is_err() {
                    return false;
                }
            }
            WireType::SixtyFourBit => {
                if buf.len() < 8 {
                    return false;
                }
                buf = &buf[8..];
            }
            WireType::LengthDelimited => {
                let length = match decode_varint(&mut buf) {
                    Ok(v) => v as usize,
                    Err(_) => return false,
                };
                if buf.len() < length {
                    return false;
                }
                buf = &buf[length..];
            }
            WireType::ThirtyTwoBit => {
                if buf.len() < 4 {
                    return false;
                }
                buf = &buf[4..];
            }
            WireType::StartGroup | WireType::EndGroup => {
                // Deprecated but technically valid
            }
        }

        prev_tag = tag;
        field_count += 1;
    }

    // Must have at least one valid field and consumed data cleanly
    field_count > 0
}

fn zigzag_decode(v: u64) -> i64 {
    ((v >> 1) as i64) ^ (-((v & 1) as i64))
}

fn is_printable(s: &str) -> bool {
    let printable = s.chars().filter(|c| !c.is_control() || *c == '\n' || *c == '\t').count();
    printable > s.len() * 3 / 4
}

fn hex_preview(data: &[u8], max_bytes: usize) -> String {
    let limit = data.len().min(max_bytes);
    let mut s = String::with_capacity(limit * 3 + 4);
    for (i, &b) in data[..limit].iter().enumerate() {
        if i > 0 {
            s.push(' ');
        }
        s.push(HEX_CHARS[(b >> 4) as usize] as char);
        s.push(HEX_CHARS[(b & 0xf) as usize] as char);
    }
    if data.len() > max_bytes {
        s.push_str("...");
    }
    s
}

const HEX_CHARS: &[u8; 16] = b"0123456789abcdef";

fn fmt_error(msg: &str, offset: usize) -> String {
    format!("{} at offset {}", msg, offset)
}

// --- JSON builder (same pattern as wasm-decoder-http) ---

struct JsonBuilder {
    json: String,
    first: bool,
}

impl JsonBuilder {
    fn new() -> Self {
        Self {
            json: String::from("["),
            first: true,
        }
    }

    fn entry(&mut self, label: &str, value: &str) {
        if !self.first {
            self.json.push(',');
        }
        self.first = false;
        self.json.push_str(r#"{"label":""#);
        json_escape(&mut self.json, label);
        self.json.push_str(r#"","value":""#);
        json_escape(&mut self.json, value);
        self.json.push_str(r#""}"#);
    }

    fn entry_range(&mut self, label: &str, value: &str, offset: usize, length: usize) {
        if !self.first {
            self.json.push(',');
        }
        self.first = false;
        self.json.push_str(r#"{"label":""#);
        json_escape(&mut self.json, label);
        self.json.push_str(r#"","value":""#);
        json_escape(&mut self.json, value);
        self.json.push_str(r#"","offset":"#);
        self.json.push_str(&fmt_usize(offset));
        self.json.push_str(r#","length":"#);
        self.json.push_str(&fmt_usize(length));
        self.json.push('}');
    }

    fn finish(mut self) -> i32 {
        self.json.push(']');
        self.json.push('\0');
        let bytes = self.json.into_bytes();
        let ptr = bytes.as_ptr() as i32;
        std::mem::forget(bytes);
        ptr
    }
}

fn json_escape(out: &mut String, s: &str) {
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

fn fmt_usize(v: usize) -> String {
    if v == 0 {
        return "0".into();
    }
    let mut buf = [0u8; 20];
    let mut i = 0;
    let mut n = v;
    while n > 0 {
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
        i += 1;
    }
    let mut s = String::with_capacity(i);
    while i > 0 {
        i -= 1;
        s.push(buf[i] as char);
    }
    s
}
