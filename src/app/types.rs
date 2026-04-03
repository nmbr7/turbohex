//! Type definitions for application state, input modes, and decoder configuration.

/// Determines whether the user is selecting at byte or bit granularity.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SelectionMode {
    /// Standard byte-level selection (default).
    Byte,
    /// Bit-level selection for sub-byte field inspection.
    Bit,
}

/// The current input mode of the application, controlling which key handler is active.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    /// Default browsing mode with full navigation.
    Normal,
    /// Visual selection mode: arrow keys extend the selection from the anchor point.
    Selecting,
    /// Text input mode for the "goto offset" prompt.
    GotoOffset,
    /// Modal help popup is displayed; any key dismisses it.
    Help,
    /// Decoder settings popup for enabling/disabling decoders and editing params.
    DecoderSettings,
    /// Inline text editing of a decoder parameter value.
    ParamEdit,
}

/// Describes the data type of a configurable decoder parameter.
#[derive(Clone, PartialEq, Eq)]
pub enum ParamType {
    /// Free-form string value.
    String,
    /// Integer value (validated on entry).
    Int,
    /// Boolean toggle (`"true"` / `"false"`).
    Bool,
    /// A fixed set of choices the user cycles through.
    Choice(Vec<std::string::String>),
}

/// A single configurable parameter exposed by a decoder plugin.
///
/// Decoders declare parameters via their `params()` export. Each parameter
/// has a name, type, default value, and a current value that the user can
/// modify through the decoder settings UI.
#[derive(Clone)]
pub struct DecoderParam {
    /// Display name of the parameter.
    pub name: String,
    /// The expected value type, used for validation and UI rendering.
    pub param_type: ParamType,
    /// The default value provided by the decoder plugin.
    #[allow(dead_code)]
    pub default: String,
    /// The current user-configured value (initialized to `default`).
    pub value: String,
}

/// Metadata and runtime state for a registered decoder.
///
/// Each decoder (built-in, Lua, or WASM) is tracked as a `DecoderInfo` entry
/// in the application state. Users can enable/disable decoders and modify
/// their parameters through the settings UI.
#[derive(Clone)]
pub struct DecoderInfo {
    /// Human-readable name of the decoder (e.g., `"Built-in"`, `"example"`).
    pub name: String,
    /// Where this decoder comes from.
    pub source: DecoderSource,
    /// Whether this decoder is currently active and will produce decode output.
    pub enabled: bool,
    /// User-configurable parameters for this decoder.
    pub params: Vec<DecoderParam>,
}

/// Identifies the origin of a decoder plugin.
#[derive(Clone, PartialEq, Eq)]
pub enum DecoderSource {
    /// The built-in decoder that ships with turbohex (integer, float, string, etc.).
    Builtin,
    /// A Lua script loaded from `~/.config/turbohex/decoders/*.lua`.
    Lua,
    /// A WASM module loaded from `~/.config/turbohex/decoders/*.wasm`.
    Wasm,
}
