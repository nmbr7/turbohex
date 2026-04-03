//! Decoding engine for interpreting raw bytes as typed values.
//!
//! This module contains all decoding functionality: the built-in decoders
//! (integer, float, string, timestamp), bit-level decoding, byte statistics,
//! and the Lua and WASM plugin decoder managers.
//!
//! # Submodules
//!
//! - [`types`]: Core types (`Endian`, `DecodedValue`, `RANGE_COLORS`) and helper functions.
//! - [`builtin`]: Built-in byte-level decoder for common data types.
//! - [`bits`]: Bit-level decoder for sub-byte field inspection.
//! - [`stats`]: Byte-level statistical analysis (entropy, sparsity, uniqueness).
//! - [`lua`]: Lua decoder plugin manager.
//! - [`wasm`]: WASM decoder plugin manager.

pub mod bits;
pub mod builtin;
pub mod lua;
pub mod stats;
pub mod types;
pub mod wasm;

pub use bits::decode_bits;
pub use builtin::decode_selection;
pub use lua::LuaDecoderManager;
pub use stats::byte_stats;
pub use types::{DecodedValue, Endian, RANGE_COLORS};
pub use wasm::WasmDecoderManager;
