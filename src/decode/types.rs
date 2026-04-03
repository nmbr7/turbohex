//! Core types for the decoding system.
//!
//! These types are shared across the built-in decoder, Lua/WASM plugin decoders,
//! and the UI rendering layer.

use std::fmt::Write;

/// Byte order for multi-byte integer and float decoding.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Endian {
    /// Least significant byte first (x86, ARM default).
    Little,
    /// Most significant byte first (network byte order).
    Big,
}

impl Endian {
    /// Returns a short display label: `"LE"` or `"BE"`.
    pub fn label(&self) -> &str {
        match self {
            Endian::Little => "LE",
            Endian::Big => "BE",
        }
    }
}

/// A single decoded field produced by a decoder.
///
/// Each `DecodedValue` represents one line in the decode panel. If `range`
/// is set, the field maps to a specific byte range within the selection,
/// enabling color-coded highlighting in the hex view. Entries with empty
/// `label` and no `range` serve as visual spacers.
#[derive(Clone)]
pub struct DecodedValue {
    /// Display label shown in the decode panel (e.g., `"u32 LE"`, `"ASCII"`).
    pub label: String,
    /// The decoded value as a display string.
    pub value: String,
    /// Byte range relative to the selection start: `(offset, length)`.
    /// Used for color-coded range highlighting in the hex view.
    pub range: Option<(usize, usize)>,
    /// Color palette index assigned by the rendering layer for range visualization.
    pub color_index: Option<usize>,
}

impl DecodedValue {
    /// Creates a new `DecodedValue` with no range mapping.
    pub(crate) fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            range: None,
            color_index: None,
        }
    }

    /// Attaches a byte range to this decoded value for hex view highlighting.
    ///
    /// The range is relative to the selection start: `offset` is the number of
    /// bytes from the start of the selection, and `length` is the field size.
    #[allow(dead_code)]
    pub fn with_range(mut self, offset: usize, length: usize) -> Self {
        self.range = Some((offset, length));
        self
    }
}

/// A color palette for mapping decoder field ranges to distinct colors.
///
/// Each entry is an `(R, G, B)` tuple. The palette is cycled through when
/// assigning colors to range-mapped decode entries. Ten colors provide
/// sufficient contrast for typical wire format decoders.
pub const RANGE_COLORS: &[(u8, u8, u8)] = &[
    (100, 180, 240), // blue
    (240, 160, 80),  // orange
    (120, 220, 120), // green
    (220, 120, 220), // purple
    (240, 220, 80),  // yellow
    (100, 220, 220), // cyan
    (240, 120, 120), // red
    (180, 140, 240), // lavender
    (140, 220, 180), // teal
    (240, 180, 160), // salmon
];

/// Reads a 16-bit unsigned integer from the first 2 bytes of `bytes` with the given endianness.
pub(crate) fn read_u16(bytes: &[u8], endian: Endian) -> u16 {
    let b: [u8; 2] = [bytes[0], bytes[1]];
    match endian {
        Endian::Little => u16::from_le_bytes(b),
        Endian::Big => u16::from_be_bytes(b),
    }
}

/// Reads a 32-bit unsigned integer from the first 4 bytes of `bytes` with the given endianness.
pub(crate) fn read_u32(bytes: &[u8], endian: Endian) -> u32 {
    let b: [u8; 4] = [bytes[0], bytes[1], bytes[2], bytes[3]];
    match endian {
        Endian::Little => u32::from_le_bytes(b),
        Endian::Big => u32::from_be_bytes(b),
    }
}

/// Reads a 64-bit unsigned integer from the first 8 bytes of `bytes` with the given endianness.
pub(crate) fn read_u64(bytes: &[u8], endian: Endian) -> u64 {
    let b: [u8; 8] = [
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ];
    match endian {
        Endian::Little => u64::from_le_bytes(b),
        Endian::Big => u64::from_be_bytes(b),
    }
}

/// Formats a byte slice as a space-separated hex string, truncated after 32 bytes.
pub(crate) fn format_hex(bytes: &[u8]) -> String {
    let mut hex = String::new();
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 {
            hex.push(' ');
        }
        write!(hex, "{:02X}", b).ok();
        if i >= 31 {
            hex.push_str("...");
            break;
        }
    }
    hex
}

/// Formats a byte slice as a space-separated binary string, truncated after 8 bytes.
pub(crate) fn format_binary(bytes: &[u8]) -> String {
    let mut bin = String::new();
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 {
            bin.push(' ');
        }
        write!(bin, "{:08b}", b).ok();
        if i >= 7 {
            bin.push_str("...");
            break;
        }
    }
    bin
}

/// Formats a byte slice as a space-separated octal string, truncated after 16 bytes.
pub(crate) fn format_octal(bytes: &[u8]) -> String {
    let mut oct = String::new();
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 {
            oct.push(' ');
        }
        write!(oct, "{:03o}", b).ok();
        if i >= 15 {
            oct.push_str("...");
            break;
        }
    }
    oct
}

