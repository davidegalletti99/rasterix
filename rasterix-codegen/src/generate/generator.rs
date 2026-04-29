use proc_macro2::TokenStream;
use quote::quote;

use crate::transform::{lowerer, ir::IR, lower_ir::LoweredIR};
use super::{item_gen::generate_item, record_gen::generate_record, datablock_gen::generate_datablock};

/// Main code generation orchestrator.
///
/// Lowers the semantic IR into a flat representation, then produces
/// a complete Rust module containing:
/// - All necessary imports
/// - Category record struct (Cat{N}Record)
/// - All item structs (Item{N})
/// - All enum definitions
/// - All decode/encode implementations
///
/// # Arguments
///
/// * `ir` - The validated intermediate representation
///
/// # Returns
///
/// A TokenStream containing the complete generated module.
pub fn generate(ir: &IR) -> TokenStream {
    let lowered = lowerer::lower(ir);
    generate_from_lowered(&lowered)
}

fn generate_from_lowered(lowered: &LoweredIR) -> TokenStream {
    let module_name = &lowered.module_name;

    let record = generate_record(&lowered.record);
    let datablock = generate_datablock(lowered);

    let items: Vec<_> = lowered.items.iter()
        .map(generate_item)
        .collect();

    quote! {
        // AUTO-GENERATED CODE — DO NOT EDIT
        //
        // This file was automatically generated from ASTERIX XML definitions.
        // Manual modifications will be lost on regeneration.

        #![allow(unused_imports)]
        #![allow(dead_code)]

        use rasterix::rcore::{BitReader, BitWriter, DecodeError, Fspec, Decode, Encode};
        use std::io::{Read, Write};

        pub mod #module_name {
            use super::*;
            // Category record
            #record

            // Data block
            #datablock

            // Data items
            #(#items)*
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transform::ir::*;

    #[test]
    fn test_generate_complete_module() {
        let ir = IR {
            category: IRCategory {
                id: 48,
                items: vec![
                    IRItem {
                        id: 10,
                        frn: 1,
                        layout: IRLayout::Fixed {
                            bytes: 2,
                            elements: vec![
                                IRElement::Field {
                                    name: "sac".to_string(),
                                    bits: 8,
                                    is_string: false,
                                },
                                IRElement::Field {
                                    name: "sic".to_string(),
                                    bits: 8,
                                    is_string: false,
                                },
                            ],
                        },
                    },
                ],
            },
        };

        let result = generate(&ir);
        let code = result.to_string();

        // Check for imports (quote! adds spaces around :: and braces)
        assert!(code.contains("use rasterix :: rcore"));
        assert!(code.contains("Decode"));
        assert!(code.contains("Encode"));

        // Check for record
        assert!(code.contains("pub struct Record"));

        // Check for data block
        assert!(code.contains("pub struct DataBlock"));
        assert!(code.contains("impl Encode for DataBlock"));
        assert!(code.contains("impl Decode for DataBlock"));

        // Check for item
        assert!(code.contains("pub struct Item010"));
        assert!(code.contains("pub sac : u8"));
        assert!(code.contains("pub sic : u8"));
    }
}
