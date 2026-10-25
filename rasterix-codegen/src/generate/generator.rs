use proc_macro2::TokenStream;
use quote::quote;

use crate::transform::ir::IR;
use super::{
    datablock_gen::generate_datablock, item_gen::generate_item, record_gen::generate_record,
    utils::{item_struct_name, module_name},
};

/// Main code generation orchestrator.
///
/// Produces a complete Rust module containing:
/// - All necessary imports
/// - Category record struct
/// - All item structs (Item{N})
/// - All enum definitions
/// - All decode/encode implementations
///
/// Takes a validated [`IR`] — every structural invariant the generators rely
/// on is checked by [`IR::new`], so generation itself cannot fail.
pub fn generate(ir: &IR) -> TokenStream {
    let category = &ir.category;
    let module_name = module_name(category.id);

    let record = generate_record(category);
    let datablock = generate_datablock(category.id);

    let items: Vec<_> = category.items.iter()
        .map(|item| generate_item(&item_struct_name(item.id), &item.layout))
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
    use test_utils::assert_code_contains;

    #[test]
    fn test_generate_complete_module() {
        let elements = vec![
            IRElement::Field {
                name: "sac".to_string(),
                bits: 8,
                string: None,
            },
            IRElement::Field {
                name: "sic".to_string(),
                bits: 8,
                string: None,
            },
        ];
        let items = vec![
            IRItem {
                id: 10,
                frn: 1,
                layout: IRLayout::Fixed {
                    bytes: 2,
                    elements,
                },
            },
        ];
        let ir = IR::new(48, items).expect("the fixture is valid");

        let code = generate(&ir).to_string();

        let expected_fragments = [
            // Check for imports (quote! adds spaces around :: and braces)
            "use rasterix :: rcore",
            "Decode",
            "Encode",
            // Check for module
            "pub mod cat048",
            // Check for record
            "pub struct Record",
            // Check for data block
            "pub struct DataBlock",
            "impl Encode for DataBlock",
            "impl Decode for DataBlock",
            // Check for item and fields
            "pub struct Item010",
            "pub sac : u8",
            "pub sic : u8",
        ];
        assert_code_contains(&code, &expected_fragments);
    }
}
