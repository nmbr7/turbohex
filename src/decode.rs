use std::fmt::Write;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Endian {
    Little,
    Big,
}

impl Endian {
    pub fn label(&self) -> &str {
        match self {
            Endian::Little => "LE",
            Endian::Big => "BE",
        }
    }
}

/// A single decoded field. If `range` is set, it maps to a byte range
/// (relative to the selection start) that produced this value.
/// Decoders for wire formats can return multiple entries with ranges
/// to color-code different fields in the hex view.
#[derive(Clone)]
pub struct DecodedValue {
    pub label: String,
    pub value: String,
    /// Byte range relative to selection start: (offset, length)
    pub range: Option<(usize, usize)>,
    /// Which decoder group this belongs to (set by the rendering layer)
    pub color_index: Option<usize>,
}

impl DecodedValue {
    fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            range: None,
            color_index: None,
        }
    }

    pub fn with_range(mut self, offset: usize, length: usize) -> Self {
        self.range = Some((offset, length));
        self
    }
}

/// A color palette for mapping decoder field ranges to distinct colors.
pub const RANGE_COLORS: &[(u8, u8, u8)] = &[
    (100, 180, 240),  // blue
    (240, 160, 80),   // orange
    (120, 220, 120),  // green
    (220, 120, 220),  // purple
    (240, 220, 80),   // yellow
    (100, 220, 220),  // cyan
    (240, 120, 120),  // red
    (180, 140, 240),  // lavender
    (140, 220, 180),  // teal
    (240, 180, 160),  // salmon
];

pub fn decode_selection(bytes: &[u8], endian: Endian) -> Vec<DecodedValue> {
    let mut results = Vec::new();

    if bytes.is_empty() {
        return results;
    }

    // Raw hex
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
    results.push(DecodedValue::new("Hex", hex));

    // Binary
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
    results.push(DecodedValue::new("Binary", bin));

    // Octal
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
    results.push(DecodedValue::new("Octal", oct));

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

        // Unix timestamp
        let ts = v as i64;
        if (0..=4_102_444_800).contains(&ts) {
            // Up to year 2100
            let secs = ts;
            let days = secs / 86400;
            let time_of_day = secs % 86400;
            let hours = time_of_day / 3600;
            let minutes = (time_of_day % 3600) / 60;
            let seconds = time_of_day % 60;
            // Simple epoch day calculation (from 1970-01-01)
            let (year, month, day) = epoch_days_to_date(days);
            results.push(DecodedValue::new(
                "Timestamp",
                format!(
                    "{:04}-{:02}-{:02} {:02}:{:02}:{:02} UTC",
                    year, month, day, hours, minutes, seconds
                ),
            ));
        }

        // Also try as u32 timestamp
        let ts32 = read_u32(bytes, endian) as i64;
        if (0..=4_102_444_800).contains(&ts32) {
            let secs = ts32;
            let days = secs / 86400;
            let time_of_day = secs % 86400;
            let hours = time_of_day / 3600;
            let minutes = (time_of_day % 3600) / 60;
            let seconds = time_of_day % 60;
            let (year, month, day) = epoch_days_to_date(days);
            results.push(DecodedValue::new(
                "Timestamp32",
                format!(
                    "{:04}-{:02}-{:02} {:02}:{:02}:{:02} UTC",
                    year, month, day, hours, minutes, seconds
                ),
            ));
        }
    } else if bytes.len() >= 4 {
        let ts = read_u32(bytes, endian) as i64;
        if (0..=4_102_444_800).contains(&ts) {
            let secs = ts;
            let days = secs / 86400;
            let time_of_day = secs % 86400;
            let hours = time_of_day / 3600;
            let minutes = (time_of_day % 3600) / 60;
            let seconds = time_of_day % 60;
            let (year, month, day) = epoch_days_to_date(days);
            results.push(DecodedValue::new(
                "Timestamp",
                format!(
                    "{:04}-{:02}-{:02} {:02}:{:02}:{:02} UTC",
                    year, month, day, hours, minutes, seconds
                ),
            ));
        }
    }

    results.push(DecodedValue::new("", "")); // spacer

    // String decodes
    let ascii: String = bytes
        .iter()
        .take(64)
        .map(|&b| if b.is_ascii_graphic() || b == b' ' { b as char } else { '.' })
        .collect();
    results.push(DecodedValue::new("ASCII", format!("\"{}\"", ascii)));

    let utf8 = String::from_utf8_lossy(&bytes[..bytes.len().min(64)]);
    results.push(DecodedValue::new("UTF-8", format!("\"{}\"", utf8)));

    results.push(DecodedValue::new("", "")); // spacer
    results.push(DecodedValue::new("Length", format!("{} bytes", bytes.len())));

    results
}

pub fn decode_bits(bytes: &[u8], bit_offset: usize, bit_len: usize, endian: Endian) -> Vec<DecodedValue> {
    let mut results = Vec::new();

    if bytes.is_empty() || bit_len == 0 {
        return results;
    }

    // Show the selected bits
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
    results.push(DecodedValue::new("Bit offset", format!("{}", bit_offset)));
    results.push(DecodedValue::new("Bit length", format!("{}", bit_len)));

    results
}

fn read_u16(bytes: &[u8], endian: Endian) -> u16 {
    let b: [u8; 2] = [bytes[0], bytes[1]];
    match endian {
        Endian::Little => u16::from_le_bytes(b),
        Endian::Big => u16::from_be_bytes(b),
    }
}

fn read_u32(bytes: &[u8], endian: Endian) -> u32 {
    let b: [u8; 4] = [bytes[0], bytes[1], bytes[2], bytes[3]];
    match endian {
        Endian::Little => u32::from_le_bytes(b),
        Endian::Big => u32::from_be_bytes(b),
    }
}

fn read_u64(bytes: &[u8], endian: Endian) -> u64 {
    let b: [u8; 8] = [
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ];
    match endian {
        Endian::Little => u64::from_le_bytes(b),
        Endian::Big => u64::from_be_bytes(b),
    }
}

/// Convert days since Unix epoch to (year, month, day)
fn epoch_days_to_date(days: i64) -> (i64, i64, i64) {
    // Civil from days algorithm (Howard Hinnant)
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    let y = y + if m <= 2 { 1 } else { 0 };
    (y, m, d)
}
