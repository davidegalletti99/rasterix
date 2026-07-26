//! # rasterix-core
//!
//! Core runtime library for ASTERIX message encoding and decoding.
//!
//! This crate provides the foundational types used by code generated from
//! ASTERIX XML category definitions. It has **zero external dependencies** and
//! relies only on the Rust standard library.
//!
//! ## Key components
//!
//! | Type | Purpose |
//! |------|---------|
//! | [`BitReader`] | Read individual bits from any [`std::io::Read`] source |
//! | [`BitWriter`] | Write individual bits to any [`std::io::Write`] sink |
//! | [`Fspec`] | ASTERIX Field Specification bitmap (variable-length) |
//! | [`DecodeError`] | Unified error type for encoding/decoding failures |
//!
//! ## Traits
//!
//! Generated ASTERIX data structures implement the [`Encode`] and [`Decode`]
//! traits, which operate on [`BitWriter`] / [`BitReader`] respectively.
//!
//! ## Example
//!
//! ```rust
//! use rasterix_core::{BitReader, BitWriter};
//! use std::io::Cursor;
//!
//! // Write 12 bits
//! let mut buf = Vec::new();
//! let mut writer = BitWriter::new(&mut buf);
//! writer.write_bits(0xABC, 12).unwrap();
//! writer.flush().unwrap();
//!
//! // Read them back
//! let mut reader = BitReader::new(Cursor::new(&buf));
//! assert_eq!(reader.read_bits(12).unwrap(), 0xABC);
//! ```

pub mod bit_reader;
pub mod bit_writer;
pub mod error;
pub mod fspec;

pub use bit_reader::BitReader;
pub use bit_writer::BitWriter;
pub use error::DecodeError;
pub use fspec::Fspec;

/// Trait for encoding ASTERIX data structures into a bit stream.
///
/// Implementors serialize their fields into the provided [`BitWriter`],
/// returning a [`DecodeError`] on failure.
pub trait Encode {
    fn encode<W: std::io::Write>(&self, writer: &mut BitWriter<W>) -> Result<(), DecodeError>;
}

/// Trait for decoding ASTERIX data structures from a bit stream.
///
/// Implementors reconstruct themselves from the provided [`BitReader`],
/// returning a [`DecodeError`] on failure.
pub trait Decode: Sized {
    fn decode<R: std::io::Read>(reader: &mut BitReader<R>) -> Result<Self, DecodeError>;
}


#[cfg(test)]
mod tests {}