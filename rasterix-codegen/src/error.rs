use thiserror::Error;

#[derive(Debug, Error)]
pub enum CodegenError {
    // ── I/O ──────────────────────────────────────────────────────────────
    #[error("Failed to read '{path}': {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },

    // ── XML parse ────────────────────────────────────────────────────────
    #[error("Failed to parse XML: {source}")]
    Parse {
        #[from]
        source: quick_xml::DeError,
    },

    // ── Transform / validation ───────────────────────────────────────────
    #[error("Invalid field type '{field_type}' for field '{field_name}': expected 'string' or 'numeric'")]
    InvalidFieldType { field_name: String, field_type: String },

    #[error("Invalid counter value '{value}' in repetitive item: {source}")]
    InvalidCounter {
        value: String,
        #[source]
        source: std::num::ParseIntError,
    },

    #[error("Invalid enum value '{value}' for variant '{variant}': {source}")]
    InvalidEnumValue {
        variant: String,
        value: String,
        #[source]
        source: std::num::ParseIntError,
    },

    #[error("{context}: bit count mismatch — declared {bytes} bytes ({expected_bits} bits) but elements use {actual_bits} bits")]
    BitCountMismatch {
        context: String,
        bytes: usize,
        expected_bits: usize,
        actual_bits: usize,
    },

    #[error("{context}: byte count mismatch — declared {declared} bytes but found {actual} part group(s)")]
    ExtendedByteMismatch {
        context: String,
        declared: usize,
        actual: usize,
    },

    #[error("{context}, part group {index}: has {actual} bits but must have exactly 7 data bits")]
    PartGroupBitMismatch {
        context: String,
        index: usize,
        actual: usize,
    },

    // ── Path ──────────────────────────────────────────────────────────────
    #[error("File path is not valid UTF-8")]
    InvalidPath,

    // ── Lowerer programmer-error guards ───────────────────────────────────
    #[error("Nested EPB elements are not supported")]
    NestedEpb,

    #[error("EPB can only wrap a Field or Enum, not a Spare")]
    EpbContainsSpare,

    #[error("Nested compound items are not supported")]
    NestedCompound,
}
