use std::fmt;

/// Unified error type for ASTERIX encoding and decoding failures.
///
/// # Variants
///
/// - [`Io`](Self::Io) -- wraps an underlying [`std::io::Error`] encountered
///   while reading from or writing to a byte stream.
/// - [`InvalidData`](Self::InvalidData) -- represents a logical data-format
///   error such as an unexpected value, a missing field, or a constraint
///   violation.
///
/// # Example
///
/// ```
/// use rasterix_core::DecodeError;
/// use std::io;
///
/// let io_err = DecodeError::Io(io::Error::new(io::ErrorKind::UnexpectedEof, "truncated"));
/// assert!(matches!(io_err, DecodeError::Io(_)));
///
/// let data_err = DecodeError::InvalidData("SAC out of range");
/// assert!(matches!(data_err, DecodeError::InvalidData(_)));
/// ```
#[derive(Debug)]
pub enum DecodeError {
    Io(std::io::Error),
    InvalidData(&'static str),
}

impl From<std::io::Error> for DecodeError {
    fn from(err: std::io::Error) -> Self {
        DecodeError::Io(err)
    }
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DecodeError::Io(e) => write!(f, "IO error: {e}"),
            DecodeError::InvalidData(msg) => write!(f, "Invalid data: {msg}"),
        }
    }
}
