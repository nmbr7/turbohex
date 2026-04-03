//! Built-in byte-level decoder for common data types.
//!
//! Decodes the selected bytes into multiple representations: raw hex/binary/octal,
//! integer types (u8..u64, signed and unsigned), floating point (f32, f64),
//! Unix timestamps, ASCII/UTF-8 strings, and entropy statistics.

use chrono::DateTime;

use super::stats::byte_stats;
use super::types::{
    DecodedValue, Endian, format_binary, format_hex, format_octal, read_u16, read_u32, read_u64,
};

/// Formats a Unix timestamp (seconds since epoch) as a UTC datetime string.
///
/// Returns `None` if the timestamp is outside the range 0..=4102444800 (year 2100).
fn format_timestamp(secs: i64) -> Option<String> {
    if !(0..=4_102_444_800).contains(&secs) {
        return None;
    }
    DateTime::from_timestamp(secs, 0).map(|dt| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string())
}

/// Decodes a byte slice into all applicable built-in interpretations.
///
/// The returned entries are ordered for display in the decode panel:
/// raw formats (hex, binary, octal), integer types of increasing width,
/// floating point, timestamps, string decodings, and entropy statistics.
///
/// Only types that fit within the selection length are included (e.g., u32
/// requires at least 4 bytes).
pub fn decode_selection(bytes: &[u8], endian: Endian) -> Vec<DecodedValue> {
    let mut results = Vec::new();

    if bytes.is_empty() {
        return results;
    }

    // Raw hex
    results.push(DecodedValue::new("Hex", format_hex(bytes)));

    // Binary
    results.push(DecodedValue::new("Binary", format_binary(bytes)));

    // Octal
    results.push(DecodedValue::new("Octal", format_octal(bytes)));

    results.push(DecodedValue::new("", "")); // spacer

    // Integer decodes
    if bytes.len() >= 1 {
        results.push(DecodedValue::new("u8", format!("{}", bytes[0])));
        results.push(DecodedValue::new("i8", format!("{}", bytes[0] as i8)));
    }

    if bytes.len() >= 2 {
        let v = read_u16(bytes, endian);
        results.push(DecodedValue::new(
            format!("u16 {}", endian.label()),
            format!("{}", v),
        ));
        results.push(DecodedValue::new(
            format!("i16 {}", endian.label()),
            format!("{}", v as i16),
        ));
    }

    if bytes.len() >= 4 {
        let v = read_u32(bytes, endian);
        results.push(DecodedValue::new(
            format!("u32 {}", endian.label()),
            format!("{}", v),
        ));
        results.push(DecodedValue::new(
            format!("i32 {}", endian.label()),
            format!("{}", v as i32),
        ));
        results.push(DecodedValue::new(
            format!("f32 {}", endian.label()),
            format!("{}", f32::from_bits(v)),
        ));
    }

    if bytes.len() >= 8 {
        let v = read_u64(bytes, endian);
        results.push(DecodedValue::new(
            format!("u64 {}", endian.label()),
            format!("{}", v),
        ));
        results.push(DecodedValue::new(
            format!("i64 {}", endian.label()),
            format!("{}", v as i64),
        ));
        results.push(DecodedValue::new(
            format!("f64 {}", endian.label()),
            format!("{}", f64::from_bits(v)),
        ));

        // Unix timestamp (64-bit)
        if let Some(ts_str) = format_timestamp(v as i64) {
            results.push(DecodedValue::new("Timestamp", ts_str));
        }

        // Also try as u32 timestamp
        if let Some(ts_str) = format_timestamp(read_u32(bytes, endian) as i64) {
            results.push(DecodedValue::new("Timestamp32", ts_str));
        }
    } else if bytes.len() >= 4 {
        if let Some(ts_str) = format_timestamp(read_u32(bytes, endian) as i64) {
            results.push(DecodedValue::new("Timestamp", ts_str));
        }
    }

    results.push(DecodedValue::new("", "")); // spacer

    // String decodes
    let ascii: String = bytes
        .iter()
        .take(64)
        .map(|&b| {
            if b.is_ascii_graphic() || b == b' ' {
                b as char
            } else {
                '.'
            }
        })
        .collect();
    results.push(DecodedValue::new("ASCII", format!("\"{}\"", ascii)));

    let utf8 = String::from_utf8_lossy(&bytes[..bytes.len().min(64)]);
    results.push(DecodedValue::new("UTF-8", format!("\"{}\"", utf8)));

    results.push(DecodedValue::new("", "")); // spacer
    results.push(DecodedValue::new(
        "Length",
        format!("{} bytes", bytes.len()),
    ));

    // Entropy & sparsity stats
    if bytes.len() >= 2 {
        let stats = byte_stats(bytes);
        results.push(DecodedValue::new("Entropy", stats.entropy_display()));
        results.push(DecodedValue::new("Compress", stats.compress_display()));
        results.push(DecodedValue::new("Sparsity", stats.sparsity_display()));
        results.push(DecodedValue::new("Unique", stats.unique_display()));
    }

    results
}
