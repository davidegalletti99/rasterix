use proc_macro2::TokenStream;
use quote::quote;

use crate::transform::lower_ir::LoweredRecord;

/// Generates the data Record struct and its implementations.
///
/// The record struct contains all items as Option fields, with an FSPEC
/// that is automatically managed during decode/encode.
pub fn generate_record(record: &LoweredRecord) -> TokenStream {
    let record_name = &record.name;

    let fields: Vec<_> = record.entries.iter().map(|entry| {
        let field_name = &entry.field_name;
        let item_type = &entry.type_name;
        quote! {
            pub #field_name: Option<#item_type>
        }
    }).collect();

    let decode_impl = generate_record_decode(record);
    let encode_impl = generate_record_encode(record);

    quote! {
        /// ASTERIX Category record.
        ///
        /// Contains optional data items, each controlled by a bit in the FSPEC.
        #[derive(Debug, Clone, PartialEq)]
        pub struct #record_name {
            #(#fields),*
        }

        #decode_impl

        #encode_impl
    }
}

fn generate_record_decode(record: &LoweredRecord) -> TokenStream {
    let record_name = &record.name;

    let decode_fields: Vec<_> = record.entries.iter().map(|entry| {
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

    quote! {
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
    }
}

fn generate_record_encode(record: &LoweredRecord) -> TokenStream {
    let record_name = &record.name;

    let fspec_setup: Vec<_> = record.entries.iter().map(|entry| {
        let field_name = &entry.field_name;
        let byte = entry.fspec_byte;
        let bit = entry.fspec_bit;

        quote! {
            if self.#field_name.is_some() {
                fspec.set(#byte, #bit);
            }
        }
    }).collect();

    let encode_items: Vec<_> = record.entries.iter().map(|entry| {
        let field_name = &entry.field_name;

        quote! {
            if let Some(ref item) = self.#field_name {
                item.encode(writer)?;
            }
        }
    }).collect();

    quote! {
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
    use quote::format_ident;
    use test_utils::assert_code_contains;
    use crate::transform::lower_ir::RecordEntry;

    #[test]
    fn test_generate_record() {
        let record = LoweredRecord {
            name: format_ident!("Record"),
            entries: vec![
                RecordEntry {
                    field_name: format_ident!("item010"),
                    type_name: format_ident!("Item010"),
                    fspec_byte: 0,
                    fspec_bit: 0,
                },
                RecordEntry {
                    field_name: format_ident!("item020"),
                    type_name: format_ident!("Item020"),
                    fspec_byte: 0,
                    fspec_bit: 1,
                },
            ],
        };

        
        let result = generate_record(&record);
        let code = result.to_string();

        let expected_fragments = vec![
            "pub struct Record",
            "pub item010 : Option < Item010 >",
            "pub item020 : Option < Item020 >",
            "impl Decode for Record",
            "impl Encode for Record",
        ];
        assert_code_contains(&code, &expected_fragments);
    }
}
