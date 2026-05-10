use thiserror::Error;

/// All errors that can occur during ASTERIX code generation.
///
/// Returned by every function in the public pipeline:
/// [`parse_category`](crate::parse::parser::parse_category),
/// [`to_ir`](crate::transform::transformer::to_ir),
/// [`generate`](crate::generate::generate), and the [`Builder`](crate::builder::Builder) trait.
#[derive(Debug, Error)]
pub enum CodegenError {
    // ── I/O ──────────────────────────────────────────────────────────────
    /// Failed to read or write a file at `path`.
    #[error("I/O error on '{path}': {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },

    // ── XML parse ────────────────────────────────────────────────────────
    /// The XML input could not be deserialized into the expected structure.
    #[error("Failed to parse XML: {source}")]
    Parse {
        #[from]
        source: quick_xml::DeError,
    },

    // ── Transform / validation ───────────────────────────────────────────
    /// A `<field>` element has a `type` attribute that is neither `"string"` nor `"numeric"`.
    #[error("Invalid field type '{field_type}' for field '{field_name}': expected 'string' or 'numeric'")]
    InvalidFieldType { field_name: String, field_type: String },

    /// The counter value in a `<repetitive>` item is not a valid integer.
    #[error("Invalid counter value '{value}' in repetitive item: {source}")]
    InvalidCounter {
        value: String,
        #[source]
        source: std::num::ParseIntError,
    },

    /// An `<enum>` variant's `value` attribute is not a valid integer.
    #[error("Invalid enum value '{value}' for variant '{variant}': {source}")]
    InvalidEnumValue {
        variant: String,
        value: String,
        #[source]
        source: std::num::ParseIntError,
    },

    /// The declared byte count doesn't match the sum of element bit widths.
    #[error("{context}: bit count mismatch — declared {bytes} bytes ({expected_bits} bits) but elements use {actual_bits} bits")]
    BitCountMismatch {
        context: String,
        bytes: usize,
        expected_bits: usize,
        actual_bits: usize,
    },

    /// The declared byte count for an `<extended>` item doesn't match the number of part groups.
    #[error("{context}: byte count mismatch — declared {declared} bytes but found {actual_groups} part group(s)")]
    ExtendedByteMismatch {
        context: String,
        declared: usize,
        actual_groups: usize,
    },

    /// A part group in an `<extended>` item doesn't have exactly 7 data bits.
    #[error("{context}, part group {index}: has {actual} bits but must have exactly 7 data bits")]
    PartGroupBitMismatch {
        context: String,
        index: usize,
        actual: usize,
    },

    // ── Path ──────────────────────────────────────────────────────────────
    /// A file path contains non-UTF-8 bytes and cannot be used.
    #[error("File path is not valid UTF-8")]
    InvalidPath,

    // ── Lowerer programmer-error guards ───────────────────────────────────
    /// An EPB element contains another EPB element, which is not supported.
    #[error("Nested EPB elements are not supported")]
    NestedEpb,

    /// An EPB element wraps a `<spare>` element; only `<field>` and `<enum>` are allowed.
    #[error("EPB can only wrap a Field or Enum, not a Spare")]
    EpbContainsSpare,

    /// A `<compound>` item contains another `<compound>` item, which is not supported.
    #[error("Nested compound items are not supported")]
    NestedCompound,
}
