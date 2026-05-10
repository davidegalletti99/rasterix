//! Code generation library for ASTERIX message definitions.
//!
//! Parses ASTERIX XML category definitions and generates type-safe Rust structs with
//! [`Encode`](rasterix_core::Encode) / [`Decode`](rasterix_core::Decode) implementations.
//!
//! The pipeline runs in three stages: [`parse`] → [`transform`] → [`generate`].
//! Every stage returns `Result<_, `[`CodegenError`]`>`.
//!
//! # Quick start
//!
//! ```no_run
//! use rasterix_codegen::builder::{Builder, RustBuilder};
//!
//! let code = RustBuilder::new().build("cat048.xml")?;
//! std::fs::write("cat048.rs", code)?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

pub mod generate;
pub mod transform;
pub mod parse;
pub mod builder;
pub mod error;

pub use error::CodegenError;
