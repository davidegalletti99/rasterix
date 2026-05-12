use proc_macro2::{Ident, TokenStream};
use quote::quote;

use crate::transform::lower_ir::{EncodeOp, LoweredPart};

/// Emits a single encode operation as a TokenStream.
fn emit_encode_op(op: &EncodeOp) -> TokenStream {
    match op {
        EncodeOp::WriteField { name, bits } => {
            quote! {
                writer.write_bits(self.#name as u64, #bits)?;
            }
        }
        EncodeOp::WriteEnum { name, bits } => {
            quote! {
                writer.write_bits(u8::from(self.#name) as u64, #bits)?;
            }
        }
        EncodeOp::WriteEpbField { name, bits } => {
            quote! {
                if let Some(value) = self.#name {
                    writer.write_bits(1, 1)?; // Valid bit
                    writer.write_bits(value as u64, #bits)?;
                } else {
                    writer.write_bits(0, 1)?; // Invalid bit
                    writer.write_bits(0, #bits)?; // Zero value
                }
            }
        }
        EncodeOp::WriteEpbEnum { name, bits } => {
            quote! {
                if let Some(value) = self.#name {
                    writer.write_bits(1, 1)?; // Valid bit
                    writer.write_bits(u8::from(value) as u64, #bits)?;
                } else {
                    writer.write_bits(0, 1)?; // Invalid bit
                    writer.write_bits(0, #bits)?; // Zero value
                }
            }
        }
        EncodeOp::WriteString { name, byte_len } => {
            quote! {
                writer.write_string(&self.#name, #byte_len)?;
            }
        }
        EncodeOp::WriteEpbString { name, byte_len } => {
            quote! {
                if let Some(ref value) = self.#name {
                    writer.write_bits(1, 1)?; // Valid bit
                    writer.write_string(value, #byte_len)?;
                } else {
                    writer.write_bits(0, 1)?; // Invalid bit
                    writer.write_string("", #byte_len)?; // Write empty padded string
                }
            }
        }
        EncodeOp::WriteSpare { bits } => {
            quote! {
                writer.write_bits(0, #bits)?; // Write spare bits as zero
            }
        }
        EncodeOp::WriteLengthByte { total_bytes } => {
            quote! {
                writer.write_bits(#total_bytes as u64, 8)?;
            }
        }
    }
}

/// Generates the Encode impl for a Simple (Fixed/Explicit) item.
pub fn generate_simple_encode(
    name: &Ident,
    encode_ops: &[EncodeOp],
) -> TokenStream {
    let op_tokens: Vec<_> = encode_ops.iter().map(emit_encode_op).collect();

    quote! {
        impl Encode for #name {
            fn encode<W: std::io::Write>(
                &self,
                writer: &mut BitWriter<W>,
            ) -> Result<(), DecodeError> {
                #(#op_tokens)*
                Ok(())
            }
        }
    }
}

/// Generates encode implementations for an Extended item.
pub fn generate_extended_encode(
    name: &Ident,
    parts: &[LoweredPart],
) -> TokenStream {
    let mut part_impl_tokens = Vec::new();
    let mut main_encode_body = Vec::new();
    let total_parts = parts.len();

    for (i, part) in parts.iter().enumerate() {
        let part_name = &part.struct_name;
        let field_name = &part.field_name;

        let element_encodes: Vec<_> = part.encode_ops.iter().map(emit_encode_op).collect();

        part_impl_tokens.push(quote! {
            impl #part_name {
                pub fn encode<W: std::io::Write>(
                    &self,
                    writer: &mut BitWriter<W>,
                ) -> Result<(), DecodeError> {
                    #(#element_encodes)*
                    Ok(())
                }
            }
        });

        if i == 0 {
            if total_parts > 1 {
                let next_field = &parts[i + 1].field_name;
                main_encode_body.push(quote! {
                    self.#field_name.encode(writer)?;
                    writer.write_bits(self.#next_field.is_some() as u64, 1)?; // FX bit
                });
            } else {
                main_encode_body.push(quote! {
                    self.#field_name.encode(writer)?;
                    writer.write_bits(0, 1)?; // FX bit = 0, no extension
                });
            }
        } else if i < total_parts - 1 {
            let next_field = &parts[i + 1].field_name;
            main_encode_body.push(quote! {
                if let Some(ref part_data) = self.#field_name {
                    part_data.encode(writer)?;
                    writer.write_bits(self.#next_field.is_some() as u64, 1)?; // FX bit
                }
            });
        } else {
            main_encode_body.push(quote! {
                if let Some(ref part_data) = self.#field_name {
                    part_data.encode(writer)?;
                    writer.write_bits(0, 1)?; // FX bit = 0, no more extension
                }
            });
        }
    }

    quote! {
        #(#part_impl_tokens)*

        impl Encode for #name {
            fn encode<W: std::io::Write>(
                &self,
                writer: &mut BitWriter<W>,
            ) -> Result<(), DecodeError> {
                #(#main_encode_body)*
                Ok(())
            }
        }
    }
}

/// Generates encode implementation for a Repetitive item.
///
/// Wire format: [counter: counter_bytes bytes][element 0]...[element N-1]
pub fn generate_repetitive_encode(
    name: &Ident,
    counter_bytes: usize,
    element_type_name: &Ident,
    encode_ops: &[EncodeOp],
) -> TokenStream {
    let element_encodes: Vec<_> = encode_ops.iter().map(emit_encode_op).collect();
    let counter_bits = counter_bytes * 8;

    quote! {
        impl #element_type_name {
            fn encode<W: std::io::Write>(
                &self,
                writer: &mut BitWriter<W>,
            ) -> Result<(), DecodeError> {
                #(#element_encodes)*
                Ok(())
            }
        }

        impl Encode for #name {
            fn encode<W: std::io::Write>(
                &self,
                writer: &mut BitWriter<W>,
            ) -> Result<(), DecodeError> {
                // Write the repetition counter
                writer.write_bits(self.items.len() as u64, #counter_bits)?;
                for item in &self.items {
                    item.encode(writer)?;
                }
                Ok(())
            }
        }
    }
}

