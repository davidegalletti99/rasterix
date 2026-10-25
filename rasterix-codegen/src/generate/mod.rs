//! Turns a validated [`IR`](crate::transform::ir::IR) into Rust code.
//!
//! Each sub-module emits one kind of declaration directly from the IR:
//!
//! - `generator`: main orchestration, produces the complete module
//! - `record_gen`: the category record
//! - `datablock_gen`: the data block container
//! - `item_gen`: one item, including compound sub-items
//! - `struct_gen`: struct declarations
//! - `decode_gen` / `encode_gen`: the codec implementations
//! - `enum_gen`: enum types, hoisted to module level
//! - `utils`: generated names, field resolution and type mappings
pub mod generator;
pub mod record_gen;
pub mod datablock_gen;
pub mod item_gen;
pub mod struct_gen;
pub mod decode_gen;
pub mod encode_gen;
pub mod enum_gen;
pub mod utils;

pub use generator::generate;
