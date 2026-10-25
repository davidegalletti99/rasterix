use proc_macro2::{Ident, TokenStream};
use quote::quote;

use crate::transform::ir::{IRLayout, IRSubItem};
use super::{
    decode_gen::generate_decode,
    encode_gen::generate_encode,
    enum_gen::{collect_enums, generate_enum},
    struct_gen::generate_structs,
    utils::{frn_to_fspec_position, sub_field_name, sub_struct_name},
};

/// Generates all code for a single ASTERIX item: its enums, structs and codec.
pub fn generate_item(name: &Ident, layout: &IRLayout) -> TokenStream {
    match layout {
        IRLayout::Compound { sub_items } => generate_compound_item(name, sub_items),
        _ => {
            let enums = collect_enums(layout).iter().map(generate_enum).collect::<Vec<_>>();
            let structs = generate_structs(name, layout);
            let decode = generate_decode(name, layout);
            let encode = generate_encode(name, layout);
            quote! { #(#enums)* #structs #decode #encode }
        }
    }
}

/// Generates a compound item: an FSPEC followed by whichever sub-items it
/// marks present.
///
/// Each sub-item is a complete item in its own right, so it is generated the
/// same way; only the container differs.
fn generate_compound_item(name: &Ident, sub_items: &[IRSubItem]) -> TokenStream {
    let mut sub_code = Vec::new();
    let mut main_fields = Vec::new();
    let mut field_names = Vec::new();
    let mut fspec_setup = Vec::new();
    let mut sub_decodes = Vec::new();
    let mut sub_encodes = Vec::new();

    for sub in sub_items {
        let sub_name = sub_struct_name(name, sub.index);
        let field_name = sub_field_name(sub.index);
        let (byte, bit) = frn_to_fspec_position(sub.index);

        sub_code.push(generate_item(&sub_name, &sub.layout));

        main_fields.push(quote! { pub #field_name: Option<#sub_name> });

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

        field_names.push(field_name);
    }

    quote! {
        #(#sub_code)*

        #[derive(Debug, Clone, PartialEq)]
        pub struct #name {
            #(#main_fields),*
        }

        impl #name {
            pub fn decode<R: std::io::Read>(
                reader: &mut R,
            ) -> Result<Self, DecodeError> {
                let fspec = Fspec::read(reader)?;
                let mut reader = BitReader::new(reader);
                #(#sub_decodes)*
                Ok(Self { #(#field_names),* })
            }

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
    use crate::transform::ir::IRElement;
    use quote::format_ident;
    use test_utils::assert_code_contains;

    #[test]
    fn test_generate_simple_item() {
        let layout = IRLayout::Fixed {
            bytes: 2,
            elements: vec![
                IRElement::Field { name: "sac".into(), bits: 8, string: None },
                IRElement::Field { name: "sic".into(), bits: 8, string: None },
            ],
        };

        let code = generate_item(&format_ident!("Item010"), &layout).to_string();
        assert_code_contains(&code, &[
            "pub struct Item010",
            "pub sac : u8",
            "pub sic : u8",
            "impl Decode for Item010",
            "impl Encode for Item010",
        ]);
    }

    #[test]
    fn compound_sub_items_are_generated_as_items_behind_an_fspec() {
        let sub = |index: usize, field: &str| IRSubItem {
            index,
            layout: IRLayout::Fixed {
                bytes: 1,
                elements: vec![IRElement::Field { name: field.into(), bits: 8, string: None }],
            },
        };
        let layout = IRLayout::Compound { sub_items: vec![sub(1, "a"), sub(3, "b")] };

        let code = generate_item(&format_ident!("Item250"), &layout).to_string();
        assert_code_contains(&code, &[
            "pub struct Item250Sub1",
            "pub struct Item250Sub3",
            "pub sub1 : Option < Item250Sub1 >",
            "pub sub3 : Option < Item250Sub3 >",
            // index 1 → FSPEC byte 0 bit 0, index 3 → byte 0 bit 2
            "if fspec . is_set (0usize , 0u8)",
            "if fspec . is_set (0usize , 2u8)",
        ]);
    }
}
