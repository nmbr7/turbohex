//! Byte-level statistical analysis: entropy, sparsity, and uniqueness.
//!
//! These statistics are displayed both in the decode panel (for the current
//! selection) and in the stats panel (per-field breakdown).

/// Aggregate statistics for a byte sequence.
pub struct ByteStats {
    /// Shannon entropy in bits per byte (0.0 = uniform, 8.0 = maximum).
    pub entropy: f64,
    /// Number of `0x00` bytes in the sample.
    pub null_count: usize,
    /// Number of printable ASCII bytes (`0x20..=0x7E`).
    #[allow(dead_code)]
    pub printable_count: usize,
    /// Number of distinct byte values present (1..=256).
    pub unique_count: usize,
    /// Total number of bytes analyzed.
    pub total: usize,
}

impl ByteStats {
    /// Formats the entropy as a visual bar + numeric value + human-readable label.
    ///
    /// Example: `"██████░░ 5.82 bits/byte (high - binary/compiled)"`
    pub fn entropy_display(&self) -> String {
        let bar = entropy_bar(self.entropy);
        let label = entropy_label(self.entropy);
        format!("{} {:.2} bits/byte ({})", bar, self.entropy, label)
    }

    /// Formats the theoretical compression ratio as a percentage.
    ///
    /// Based on Shannon entropy: `(1 - entropy/8) * 100%` is the theoretical
    /// reduction achievable by an ideal compressor.
    pub fn compress_display(&self) -> String {
        let ratio = if self.entropy > 0.0 {
            self.entropy / 8.0
        } else {
            0.0
        };
        let reducible = ((1.0 - ratio) * 100.0) as u32;
        format!("~{}% reducible", reducible)
    }

    /// Formats the null byte sparsity as a fraction and percentage.
    pub fn sparsity_display(&self) -> String {
        let pct = if self.total > 0 {
            (self.null_count as f64 / self.total as f64 * 100.0) as u32
        } else {
            0
        };
        format!("{}/{} null bytes ({}%)", self.null_count, self.total, pct)
    }

    /// Formats the count of distinct byte values out of 256.
    pub fn unique_display(&self) -> String {
        format!("{}/256 distinct byte values", self.unique_count)
    }
}

/// Computes byte-level statistics for the given data.
///
/// Calculates Shannon entropy, null/printable byte counts, and the number
/// of distinct byte values. Returns zeroed stats for empty input.
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

    ByteStats {
        entropy,
        null_count,
        printable_count,
        unique_count,
        total,
    }
}

/// Renders a fixed-width (8-character) entropy bar using block characters.
///
/// The bar is filled proportionally to `entropy / 8.0`, where 8.0 is the
/// maximum possible Shannon entropy for a byte stream.
fn entropy_bar(entropy: f64) -> String {
    let filled = ((entropy / 8.0) * 8.0).round() as usize;
    let filled = filled.min(8);
    let empty = 8 - filled;
    format!("{}{}", "\u{2588}".repeat(filled), "\u{2591}".repeat(empty))
}

/// Returns a human-readable label classifying the entropy level.
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
