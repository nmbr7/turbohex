//! Bit-level decoder for sub-byte field inspection.
//!
//! When the user switches to bit selection mode, this decoder extracts
//! and interprets arbitrary bit ranges from the file data.

use std::fmt::Write;

use super::types::{DecodedValue, Endian};

/// Decodes an arbitrary bit range from the file data.
///
/// # Arguments
///
/// * `bytes` - The full file data (or a relevant slice).
/// * `bit_offset` - Absolute bit offset into `bytes` where the selection starts.
/// * `bit_len` - Number of selected bits.
/// * `endian` - Byte order for interpreting the extracted bit value as an integer.
///
/// # Returns
///
/// A list of decoded values showing the raw bit string, unsigned/signed integer
/// interpretations, and the bit offset/length metadata.
pub fn decode_bits(
    bytes: &[u8],
    bit_offset: usize,
    bit_len: usize,
    endian: Endian,
) -> Vec<DecodedValue> {
    let mut results = Vec::new();

    if bytes.is_empty() || bit_len == 0 {
        return results;
    }

    // Show the selected bits as a binary string
    let mut bits_str = String::new();
    for i in 0..bit_len.min(64) {
        let abs_bit = bit_offset + i;
        let byte_idx = abs_bit / 8;
        let bit_idx = 7 - (abs_bit % 8); // MSB first
        if byte_idx < bytes.len() {
            let bit = (bytes[byte_idx] >> bit_idx) & 1;
            write!(bits_str, "{}", bit).ok();
            if (i + 1) % 8 == 0 && i + 1 < bit_len {
                bits_str.push(' ');
            }
        }
    }
    results.push(DecodedValue::new("Bits", bits_str));

    // Extract bits as a u64 value (up to 64 bits)
    let mut value: u64 = 0;
    let effective_bits = bit_len.min(64);
    for i in 0..effective_bits {
        let abs_bit = bit_offset + i;
        let byte_idx = abs_bit / 8;
        let bit_idx = 7 - (abs_bit % 8);
        if byte_idx < bytes.len() {
            let bit = ((bytes[byte_idx] >> bit_idx) & 1) as u64;
            match endian {
                Endian::Big => {
                    value = (value << 1) | bit;
                }
                Endian::Little => {
                    value |= bit << i;
                }
            }
        }
    }

    results.push(DecodedValue::new("Value (uint)", format!("{}", value)));
    if effective_bits <= 63 {
        let sign_bit = 1u64 << (effective_bits - 1);
        let signed = if value >= sign_bit {
            value as i64 - (1i64 << effective_bits)
        } else {
            value as i64
        };
        results.push(DecodedValue::new("Value (int)", format!("{}", signed)));
    }

    results.push(DecodedValue::new("", ""));
    results.push(DecodedValue::new(
        "Bit offset",
        format!("{}", bit_offset),
    ));
    results.push(DecodedValue::new("Bit length", format!("{}", bit_len)));

    results
}
