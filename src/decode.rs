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

// --- Entropy & byte statistics ---

pub struct ByteStats {
    pub entropy: f64,
    pub null_count: usize,
    pub printable_count: usize,
    pub unique_count: usize,
    pub total: usize,
}

impl ByteStats {
    pub fn entropy_display(&self) -> String {
        let bar = entropy_bar(self.entropy);
        let label = entropy_label(self.entropy);
        format!("{} {:.2} bits/byte ({})", bar, self.entropy, label)
    }

    pub fn compress_display(&self) -> String {
        // Theoretical lower bound: entropy/8 of original size
        let ratio = if self.entropy > 0.0 { self.entropy / 8.0 } else { 0.0 };
        let reducible = ((1.0 - ratio) * 100.0) as u32;
        format!("~{}% reducible", reducible)
    }

    pub fn sparsity_display(&self) -> String {
        let pct = if self.total > 0 {
            (self.null_count as f64 / self.total as f64 * 100.0) as u32
        } else {
            0
        };
        format!("{}/{} null bytes ({}%)", self.null_count, self.total, pct)
    }

    pub fn unique_display(&self) -> String {
        format!("{}/256 distinct byte values", self.unique_count)
    }
}

pub fn byte_stats(bytes: &[u8]) -> ByteStats {
    let total = bytes.len();
    let mut counts = [0u32; 256];
    let mut null_count = 0usize;
    let mut printable_count = 0usize;

    for &b in bytes {
        counts[b as usize] += 1;
        if b == 0 {
            null_count += 1;
        }
        if b >= 0x20 && b <= 0x7e {
            printable_count += 1;
        }
    }

    let unique_count = counts.iter().filter(|&&c| c > 0).count();

    let entropy = if total > 0 {
        let len_f = total as f64;
        let mut h = 0.0f64;
        for &c in &counts {
            if c > 0 {
                let p = c as f64 / len_f;
                h -= p * p.log2();
            }
        }
        h
    } else {
        0.0
    };

    ByteStats { entropy, null_count, printable_count, unique_count, total }
}

fn entropy_bar(entropy: f64) -> String {
    // 8 chars wide, filled proportionally to entropy/8.0
    let filled = ((entropy / 8.0) * 8.0).round() as usize;
    let filled = filled.min(8);
    let empty = 8 - filled;
    format!("{}{}", "█".repeat(filled), "░".repeat(empty))
}

fn entropy_label(entropy: f64) -> &'static str {
    if entropy > 7.5 {
        "compressed/encrypted"
    } else if entropy > 6.0 {
        "high - binary/compiled"
    } else if entropy > 4.0 {
        "medium - structured"
    } else if entropy > 2.0 {
        "low - repetitive"
    } else {
        "very low - sparse/uniform"
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
