use proc_macro2::TokenStream;
use quote::quote;

use crate::transform::ir::IRCategory;
use super::utils::{frn_to_fspec_position, item_field_name, item_struct_name, record_struct_name};

/// One item's place in the record: its field, its type, and the FSPEC bit that
/// says whether it is present.
struct Entry {
    field_name: proc_macro2::Ident,
    type_name: proc_macro2::Ident,
    fspec_byte: usize,
    fspec_bit: u8,
}

fn entries(category: &IRCategory) -> Vec<Entry> {
    category
        .items
        .iter()
        .map(|item| {
            let (fspec_byte, fspec_bit) = frn_to_fspec_position(item.frn as usize);
            Entry {
                field_name: item_field_name(item.id),
                type_name: item_struct_name(item.id),
                fspec_byte,
                fspec_bit,
            }
        })
        .collect()
}

/// Generates the data Record struct and its implementations.
///
/// The record struct contains all items as Option fields, with an FSPEC
/// that is automatically managed during decode/encode.
pub fn generate_record(category: &IRCategory) -> TokenStream {
    let record_name = record_struct_name();
    let entries = entries(category);

    let fields: Vec<_> = entries.iter().map(|entry| {
        let field_name = &entry.field_name;
        let item_type = &entry.type_name;
        quote! {
            pub #field_name: Option<#item_type>
        }
    }).collect();

    let decode_fields: Vec<_> = entries.iter().map(|entry| {
        let field_name = &entry.field_name;
        let item_type = &entry.type_name;
        let byte = entry.fspec_byte;
        let bit = entry.fspec_bit;

        quote! {
            #field_name: if fspec.is_set(#byte, #bit) {
                Some(#item_type::decode(reader)?)
            } else {
                None
            }
        }
    }).collect();

    let fspec_setup: Vec<_> = entries.iter().map(|entry| {
        let field_name = &entry.field_name;
        let byte = entry.fspec_byte;
        let bit = entry.fspec_bit;

        quote! {
            if self.#field_name.is_some() {
                fspec.set(#byte, #bit);
            }
        }
    }).collect();

    let encode_items: Vec<_> = entries.iter().map(|entry| {
        let field_name = &entry.field_name;

        quote! {
            if let Some(ref item) = self.#field_name {
                item.encode(writer)?;
            }
        }
    }).collect();

    quote! {
        /// ASTERIX Category record.
        ///
        /// Contains optional data items, each controlled by a bit in the FSPEC.
        #[derive(Debug, Clone, PartialEq)]
        pub struct #record_name {
            #(#fields),*
        }

        impl Decode for #record_name {
            fn decode<R: std::io::Read>(
                reader: &mut BitReader<R>,
            ) -> Result<Self, DecodeError> {
                let fspec = Fspec::read(reader)?;

                Ok(Self {
                    #(#decode_fields),*
                })
            }
        }

        impl Encode for #record_name {
            fn encode<W: std::io::Write>(
                &self,
                writer: &mut BitWriter<W>,
            ) -> Result<(), DecodeError> {
                let mut fspec = Fspec::new();
                #(#fspec_setup)*
                fspec.write(writer)?;
                #(#encode_items)*
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transform::ir::{IRElement, IRItem, IRLayout};
    use test_utils::assert_code_contains;

    fn item(id: u16, frn: u8) -> IRItem {
        IRItem {
            id,
            frn,
            layout: IRLayout::Fixed {
                bytes: 1,
                elements: vec![IRElement::Field { name: "code".into(), bits: 8, string: None }],
            },
        }
    }

    #[test]
    fn test_generate_record() {
        let category = IRCategory { id: 48, items: vec![item(10, 1), item(20, 2)] };

        let code = generate_record(&category).to_string();
        assert_code_contains(&code, &[
            "pub struct Record",
            "pub item010 : Option < Item010 >",
            "pub item020 : Option < Item020 >",
            "impl Decode for Record",
            "impl Encode for Record",
        ]);
    }

    #[test]
    fn fspec_position_comes_from_the_frn() {
        // FRN 8 is the first bit of the second FSPEC byte.
        let category = IRCategory { id: 62, items: vec![item(10, 1), item(290, 8)] };

        let code = generate_record(&category).to_string();
        assert_code_contains(&code, &[
            "fspec . is_set (0usize , 0u8)",
            "fspec . is_set (1usize , 0u8)",
        ]);
    }
}
