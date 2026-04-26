use proc_macro2::{Ident, TokenStream};
use quote::quote;

use crate::transform::lower_ir::{LoweredItem, LoweredItemKind, LoweredSubItem};
use super::{
    struct_gen::*,
    decode_gen::*,
    encode_gen::*,
    enum_gen::*,
};

/// Generates all code for a single ASTERIX item from its lowered representation.
pub fn generate_item(item: &LoweredItem) -> TokenStream {
    let item_name = &item.name;
    let enum_defs: Vec<_> = item.enums.iter().map(generate_enum).collect();

    match &item.kind {
        LoweredItemKind::Simple { fields, decode_ops, encode_ops, .. } => {
            let struct_def = generate_struct(item_name, fields);
            let decode_impl = generate_simple_decode(item_name, decode_ops, fields);
            let encode_impl = generate_simple_encode(item_name, encode_ops);
            quote! { #(#enum_defs)* #struct_def #decode_impl #encode_impl }
        }
        LoweredItemKind::Extended { parts } => {
            let struct_def = generate_extended_structs(item_name, parts);
            let decode_impl = generate_extended_decode(item_name, parts);
            let encode_impl = generate_extended_encode(item_name, parts);
            quote! { #(#enum_defs)* #struct_def #decode_impl #encode_impl }
        }
        LoweredItemKind::Repetitive { element_type_name, counter_bytes, fields, decode_ops, encode_ops } => {
            let struct_def = generate_repetitive_struct(item_name, element_type_name, fields);
            let decode_impl = generate_repetitive_decode(item_name, *counter_bytes, element_type_name, decode_ops, fields);
            let encode_impl = generate_repetitive_encode(item_name, *counter_bytes, element_type_name, encode_ops);
            quote! { #(#enum_defs)* #struct_def #decode_impl #encode_impl }
        }
        LoweredItemKind::Compound { sub_items } => generate_compound_item(item_name, sub_items),
    }
}

fn generate_sub_item_struct(sub: &LoweredSubItem) -> TokenStream {
    match &sub.kind {
        LoweredItemKind::Simple { fields, .. } => generate_struct(&sub.struct_name, fields),
        LoweredItemKind::Extended { parts } => generate_extended_structs(&sub.struct_name, parts),
        LoweredItemKind::Repetitive { element_type_name, fields, .. } => {
            generate_repetitive_struct(&sub.struct_name, element_type_name, fields)
        }
        LoweredItemKind::Compound { .. } => panic!("Nested compounds not supported"),
    }
}

fn generate_sub_item_decode(sub: &LoweredSubItem) -> TokenStream {
    match &sub.kind {
        LoweredItemKind::Simple { decode_ops, fields, .. } => {
            generate_simple_decode(&sub.struct_name, decode_ops, fields)
        }
        LoweredItemKind::Extended { parts } => generate_extended_decode(&sub.struct_name, parts),
        LoweredItemKind::Repetitive { element_type_name, counter_bytes, decode_ops, fields, .. } => {
            generate_repetitive_decode(&sub.struct_name, *counter_bytes, element_type_name, decode_ops, fields)
        }
        LoweredItemKind::Compound { .. } => panic!("Nested compounds not supported"),
    }
}

fn generate_sub_item_encode(sub: &LoweredSubItem) -> TokenStream {
    match &sub.kind {
        LoweredItemKind::Simple { encode_ops, .. } => generate_simple_encode(&sub.struct_name, encode_ops),
        LoweredItemKind::Extended { parts } => generate_extended_encode(&sub.struct_name, parts),
        LoweredItemKind::Repetitive { element_type_name, counter_bytes, encode_ops, .. } => {
            generate_repetitive_encode(&sub.struct_name, *counter_bytes, element_type_name, encode_ops)
        }
        LoweredItemKind::Compound { .. } => panic!("Nested compounds not supported"),
    }
}

fn generate_compound_item(name: &Ident, sub_items: &[LoweredSubItem]) -> TokenStream {
    let mut sub_enum_defs = Vec::new();
    let mut sub_structs = Vec::new();
    let mut sub_decode_impls = Vec::new();
    let mut sub_encode_impls = Vec::new();
    let mut main_fields = Vec::new();
    let mut fspec_setup = Vec::new();
    let mut sub_decodes = Vec::new();
    let mut field_names: Vec<&Ident> = Vec::new();
    let mut sub_encodes = Vec::new();

    for sub in sub_items {
        let sub_name = &sub.struct_name;
        let field_name = &sub.field_name;
        let byte = sub.fspec_byte;
        let bit = sub.fspec_bit;

        sub_enum_defs.extend(sub.enums.iter().map(generate_enum));
        sub_structs.push(generate_sub_item_struct(sub));
        sub_decode_impls.push(generate_sub_item_decode(sub));
        sub_encode_impls.push(generate_sub_item_encode(sub));

        main_fields.push(quote! { pub #field_name: Option<#sub_name> });
        field_names.push(field_name);

        sub_decodes.push(quote! {
            let #field_name = if fspec.is_set(#byte, #bit) {
                Some(#sub_name::decode(&mut reader)?)
            } else {
                None
            };
        });

        fspec_setup.push(quote! {
            if self.#field_name.is_some() {
                fspec.set(#byte, #bit);
            }
        });

        sub_encodes.push(quote! {
            if let Some(ref sub_data) = self.#field_name {
                sub_data.encode(&mut writer)?;
            }
        });
    }

    quote! {
        #(#sub_enum_defs)*
        #(#sub_structs)*

        #[derive(Debug, Clone, PartialEq)]
        pub struct #name {
            #(#main_fields),*
        }

        #(#sub_decode_impls)*

        impl #name {
            pub fn decode<R: std::io::Read>(
                reader: &mut R,
            ) -> Result<Self, DecodeError> {
                let fspec = Fspec::read(reader)?;
                let mut reader = BitReader::new(reader);
                #(#sub_decodes)*
                Ok(Self { #(#field_names),* })
            }
        }

        #(#sub_encode_impls)*

        impl #name {
            pub fn encode<W: std::io::Write>(
                &self,
                writer: &mut W,
            ) -> Result<(), DecodeError> {
                let mut fspec = Fspec::new();
                #(#fspec_setup)*
                fspec.write(writer)?;
                let mut writer = BitWriter::new(writer);
                #(#sub_encodes)*
                writer.flush()?;
                Ok(())
            }
        }
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
