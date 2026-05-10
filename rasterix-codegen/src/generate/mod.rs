/// Code generation module.
/// 
/// This module is responsible for transforming the IR into Rust code.
/// The generation is split into several sub-modules:
/// 
/// - `generator`: Main orchestration, produces the complete output
/// - `record_gen`: Generates the Cat{N}Record struct
/// - `item_gen`: Generates Item{N} structs  
/// - `struct_gen`: Low-level struct generation utilities
/// - `decode_gen`: Generates decode implementations
/// - `encode_gen`: Generates encode implementations
/// - `enum_gen`: Generates enum types
/// - `utils`: Helper functions and type mappings
/// 
pub mod generator;
pub mod record_gen;
pub mod datablock_gen;
pub mod item_gen;
pub mod struct_gen;
pub mod decode_gen;
pub mod encode_gen;
pub mod enum_gen;
pub mod utils;

use proc_macro2::TokenStream;
use crate::error::CodegenError;
use crate::transform::ir::IR;

/// Main entry point for code generation.
///
/// Takes the validated IR and produces a complete Rust module as a TokenStream.
///
/// # Arguments
///
/// * `ir` - The intermediate representation to generate code from
///
/// # Returns
///
/// A TokenStream containing the complete generated Rust code, ready to be
/// written to a file or included in a build script.
pub fn generate(ir: &IR) -> Result<TokenStream, CodegenError> {
    generator::generate(ir)
}
