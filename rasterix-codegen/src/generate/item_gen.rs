use proc_macro2::TokenStream;
use quote::quote;

use crate::transform::lower_ir::{LoweredItem, LoweredItemKind};
use super::{
    struct_gen::*,
    decode_gen::*,
    encode_gen::*,
    enum_gen::*,
};

/// Generates all code for a single ASTERIX item from its lowered representation.
///
/// This includes:
/// - Enum definitions for any enum fields
/// - Struct definition(s) for the item
/// - Decode implementation
/// - Encode implementation
pub fn generate_item(item: &LoweredItem) -> TokenStream {
    let item_name = &item.name;

    let enum_defs: Vec<_> = item.enums.iter().map(generate_enum).collect();

    let (struct_def, decode_impl, encode_impl) = match &item.kind {
        LoweredItemKind::Simple { fields, decode_ops, encode_ops, .. } => {
            let struct_def = generate_struct(item_name, fields);
            let decode_impl = generate_simple_decode(item_name, decode_ops, fields);
            let encode_impl = generate_simple_encode(item_name, encode_ops);
            (struct_def, decode_impl, encode_impl)
        }

        LoweredItemKind::Extended { parts } => {
            let struct_def = generate_extended_structs(item_name, parts);
            let decode_impl = generate_extended_decode(item_name, parts);
            let encode_impl = generate_extended_encode(item_name, parts);
            (struct_def, decode_impl, encode_impl)
        }

        LoweredItemKind::Repetitive { element_type_name, counter_bytes, fields, decode_ops, encode_ops } => {
            let struct_def = generate_repetitive_struct(item_name, element_type_name, fields);
            let decode_impl = generate_repetitive_decode(item_name, *counter_bytes, element_type_name, decode_ops, fields);
            let encode_impl = generate_repetitive_encode(item_name, *counter_bytes, element_type_name, encode_ops);
            (struct_def, decode_impl, encode_impl)
        }

        LoweredItemKind::Compound { sub_items } => {
            // Collect enums from sub-items
            let sub_enum_defs: Vec<_> = sub_items.iter()
                .flat_map(|sub| sub.enums.iter().map(generate_enum))
                .collect();

            let struct_def = generate_compound_structs(item_name, sub_items);
            let sub_decode_impls = generate_compound_sub_decodes(sub_items);
            let sub_encode_impls = generate_compound_sub_encodes(sub_items);
            let decode_impl = generate_compound_decode(item_name, sub_items);
            let encode_impl = generate_compound_encode(item_name, sub_items);

            let combined_struct = quote! {
                #(#sub_enum_defs)*
                #struct_def
            };
            let combined_decode = quote! {
                #sub_decode_impls
                #decode_impl
            };
            let combined_encode = quote! {
                #sub_encode_impls
                #encode_impl
            };
            (combined_struct, combined_decode, combined_encode)
        }
    };

    quote! {
        #(#enum_defs)*

        #struct_def

        #decode_impl

        #encode_impl
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::format_ident;
    use crate::transform::lower_ir::*;

    #[test]
    fn test_generate_simple_item() {
        let item = LoweredItem {
            name: format_ident!("Item010"),
            enums: vec![],
            kind: LoweredItemKind::Simple {
                is_explicit: false,
                byte_size: 2,
                fields: vec![
                    FieldDescriptor {
                        name: format_ident!("sac"),
                        type_tokens: FieldType::Primitive(format_ident!("u8")),
                    },
                    FieldDescriptor {
                        name: format_ident!("sic"),
                        type_tokens: FieldType::Primitive(format_ident!("u8")),
                    },
                ],
                decode_ops: vec![
                    DecodeOp::ReadField { name: format_ident!("sac"), bits: 8, rust_type: format_ident!("u8") },
                    DecodeOp::ReadField { name: format_ident!("sic"), bits: 8, rust_type: format_ident!("u8") },
                ],
                encode_ops: vec![
                    EncodeOp::WriteField { name: format_ident!("sac"), bits: 8 },
                    EncodeOp::WriteField { name: format_ident!("sic"), bits: 8 },
                ],
            },
        };

        let result = generate_item(&item);
        let code = result.to_string();

        assert!(code.contains("pub struct Item010"));
        assert!(code.contains("pub sac : u8"));
        assert!(code.contains("pub sic : u8"));
        assert!(code.contains("impl Decode for Item010"));
        assert!(code.contains("impl Encode for Item010"));
    }
}
