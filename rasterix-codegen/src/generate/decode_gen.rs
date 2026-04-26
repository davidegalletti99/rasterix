use proc_macro2::{Ident, TokenStream};
use quote::{quote};

use crate::transform::lower_ir::{DecodeOp, FieldDescriptor, LoweredItemKind, LoweredPart, LoweredSubItem};

/// Emits a single decode operation as a TokenStream.
fn emit_decode_op(op: &DecodeOp) -> TokenStream {
    match op {
        DecodeOp::ReadField { name, bits, rust_type } => {
            quote! {
                let #name = reader.read_bits(#bits)? as #rust_type;
            }
        }
        DecodeOp::ReadEnum { name, bits, enum_type } => {
            quote! {
                let #name = {
                    let value = reader.read_bits(#bits)? as u8;
                    #enum_type::try_from(value).unwrap()
                };
            }
        }
        DecodeOp::ReadEpbField { name, bits, rust_type } => {
            quote! {
                let #name = {
                    let valid = reader.read_bits(1)? != 0;
                    if valid {
                        Some(reader.read_bits(#bits)? as #rust_type)
                    } else {
                        reader.read_bits(#bits)?; // Skip the value
                        None
                    }
                };
            }
        }
        DecodeOp::ReadEpbEnum { name, bits, enum_type } => {
            quote! {
                let #name = {
                    let valid = reader.read_bits(1)? != 0;
                    if valid {
                        let value = reader.read_bits(#bits)? as u8;
                        Some(#enum_type::try_from(value).unwrap())
                    } else {
                        reader.read_bits(#bits)?; // Skip the value
                        None
                    }
                };
            }
        }
        DecodeOp::ReadString { name, byte_len } => {
            quote! {
                let #name = reader.read_string(#byte_len)?;
            }
        }
        DecodeOp::ReadEpbString { name, byte_len } => {
            quote! {
                let #name = {
                    let valid = reader.read_bits(1)? != 0;
                    if valid {
                        Some(reader.read_string(#byte_len)?)
                    } else {
                        reader.read_string(#byte_len)?; // Skip the value
                        None
                    }
                };
            }
        }
        DecodeOp::SkipSpare { bits } => {
            quote! {
                reader.read_bits(#bits)?; // Skip spare bits
            }
        }
        DecodeOp::ReadLengthByte => {
            quote! {
                let _len = reader.read_bits(8)? as usize;
                // Length includes itself, so actual data is len - 1 bytes
            }
        }
    }
}

/// Generates the Decode impl for a Simple (Fixed/Explicit) item.
pub fn generate_simple_decode(
    name: &Ident,
    decode_ops: &[DecodeOp],
    fields: &[FieldDescriptor],
) -> TokenStream {
    let op_tokens: Vec<_> = decode_ops.iter().map(emit_decode_op).collect();
    let field_names: Vec<_> = fields.iter().map(|f| &f.name).collect();

    quote! {
        impl Decode for #name {
            fn decode<R: std::io::Read>(
                reader: &mut BitReader<R>,
            ) -> Result<Self, DecodeError> {
                #(#op_tokens)*

                Ok(Self {
                    #(#field_names),*
                })
            }
        }
    }
}

/// Generates decode implementations for an Extended item.
pub fn generate_extended_decode(
    name: &Ident,
    parts: &[LoweredPart],
) -> TokenStream {
    let mut part_impl_tokens = Vec::new();
    let mut main_decode_body = Vec::new();
    let mut field_names = Vec::new();

    let number_of_parts = parts.len();
    for (i, part) in parts.iter().enumerate() {
        let part_name = &part.struct_name;
        let field_name = &part.field_name;
        field_names.push(field_name);

        let element_decodes: Vec<_> = part.decode_ops.iter().map(emit_decode_op).collect();
        let element_names: Vec<_> = part.fields.iter().map(|f| &f.name).collect();

        part_impl_tokens.push(quote! {
            impl #part_name {
                pub fn decode<R: std::io::Read>(
                    reader: &mut BitReader<R>,
                ) -> Result<Self, DecodeError> {
                    #(#element_decodes)*
                    Ok(Self { #(#element_names),* })
                }
            }
        });

        if i == 0 {
            main_decode_body.push(quote! {
                let #field_name = #part_name::decode(reader)?;
            });

            if i != number_of_parts - 1 {
                if number_of_parts <= 2 {
                    // no need for mutable fx if there are only 2 parts
                    main_decode_body.push(quote! {
                        let fx = reader.read_bits(1)? != 0;
                    });
                } else {
                    // For more than 2 parts, we need to keep track of fx for 
                    // subsequent parts
                    main_decode_body.push(quote! {
                        let mut fx = reader.read_bits(1)? != 0;
                    });
                }
            }
        } else {
            if i != number_of_parts - 1 {
                main_decode_body.push(quote! {
                    let #field_name = if fx {
                        let part = #part_name::decode(reader)?;
                        fx = reader.read_bits(1)? != 0;
                        Some(part)
                    } else {
                        None
                    };
                });
            } else {
                main_decode_body.push(quote! {
                    let #field_name = if fx {
                        Some(#part_name::decode(reader)?)
                    } else {
                        None
                    };
                });
            }
        }
    }

    quote! {
        #(#part_impl_tokens)*

        impl Decode for #name {
            fn decode<R: std::io::Read>(
                reader: &mut BitReader<R>,
            ) -> Result<Self, DecodeError> {
                #(#main_decode_body)*

                Ok(Self {
                    #(#field_names),*
                })
            }
        }
    }
}

/// Generates decode implementation for a Repetitive item.
///
/// Wire format: [counter: counter_bytes bytes][element 0]...[element N-1]
pub fn generate_repetitive_decode(
    name: &Ident,
    counter_bytes: usize,
    element_type_name: &Ident,
    decode_ops: &[DecodeOp],
    fields: &[FieldDescriptor],
) -> TokenStream {
    let element_decodes: Vec<_> = decode_ops.iter().map(emit_decode_op).collect();
    let field_names: Vec<_> = fields.iter().map(|f| &f.name).collect();
    let counter_bits = counter_bytes * 8;

    quote! {
        impl #element_type_name {
            fn decode<R: std::io::Read>(
                reader: &mut BitReader<R>,
            ) -> Result<Self, DecodeError> {
                #(#element_decodes)*

                Ok(Self {
                    #(#field_names),*
                })
            }
        }

        impl Decode for #name {
            fn decode<R: std::io::Read>(
                reader: &mut BitReader<R>,
            ) -> Result<Self, DecodeError> {
                // Read the repetition counter
                let count = reader.read_bits(#counter_bits)? as usize;
                let mut items = Vec::with_capacity(count);
                for _ in 0..count {
                    items.push(#element_type_name::decode(reader)?);
                }

                Ok(Self { items })
            }
        }
    }
}

/// Generates decode implementation for a Compound item.
pub fn generate_compound_decode(
    name: &Ident,
    sub_items: &[LoweredSubItem],
) -> TokenStream {
    let mut sub_decodes = Vec::new();
    let mut field_names = Vec::new();

    for sub in sub_items {
        let sub_name = &sub.struct_name;
        let field_name = &sub.field_name;
        field_names.push(field_name);

        let byte = sub.fspec_byte;
        let bit = sub.fspec_bit;
        sub_decodes.push(quote! {
            let #field_name = if fspec.is_set(#byte, #bit) {
                Some(#sub_name::decode(&mut reader)?)
            } else {
                None
            };
        });
    }

    quote! {
        impl #name {
            pub fn decode<R: std::io::Read>(
                reader: &mut R,
            ) -> Result<Self, DecodeError> {
                let fspec = Fspec::read(reader)?;
                let mut reader = BitReader::new(reader);

                #(#sub_decodes)*

                Ok(Self {
                    #(#field_names),*
                })
            }
        }
    }
}

/// Generates decode implementations for all sub-items in a compound.
pub fn generate_compound_sub_decodes(
    sub_items: &[LoweredSubItem],
) -> TokenStream {
    let all_impls: Vec<_> = sub_items.iter().map(|sub| {
        match &sub.kind {
            LoweredItemKind::Simple { decode_ops, fields, .. } => {
                generate_simple_decode(&sub.struct_name, decode_ops, fields)
            }
            LoweredItemKind::Extended { parts } => {
                generate_extended_decode(&sub.struct_name, parts)
            }
            LoweredItemKind::Repetitive { element_type_name, counter_bytes, decode_ops, fields, .. } => {
                generate_repetitive_decode(&sub.struct_name, *counter_bytes, element_type_name, decode_ops, fields)
            }
            LoweredItemKind::Compound { .. } => panic!("Nested compounds not supported"),
        }
    }).collect();

    quote! {
        #(#all_impls)*
    }
}
