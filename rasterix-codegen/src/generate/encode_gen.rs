use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};

use crate::transform::ir::{IRElement, IRLayout, IRPartGroup, StringKind};
use super::utils::{
    element_struct_name, part_field_name, part_struct_name, to_snake_case, unwrap_epb,
};

/// Returns the BitWriter method used to write a string of the given kind.
fn write_string_fn(kind: StringKind) -> Ident {
    match kind {
        StringKind::Icao => format_ident!("write_string"),
        StringKind::Ascii => format_ident!("write_ascii_string"),
    }
}

/// Emits the write for one element.
///
/// An EPB-wrapped element writes its presence bit followed by its value, or a
/// cleared bit and a zeroed value — the field occupies the same bits either
/// way.
fn encode_element(element: &IRElement) -> TokenStream {
    let (inner, epb) = unwrap_epb(element);

    match inner {
        IRElement::Spare { bits } => {
            debug_assert!(!epb, "EPB-wrapped spare is rejected by IRLayout::validate");
            quote! { writer.write_bits(0, #bits)?; } // Write spare bits as zero
        }

        IRElement::Field { name, bits, string: Some(kind) } => {
            let write_fn = write_string_fn(*kind);
            let byte_len = bits / 8;
            let name = to_snake_case(name);
            if epb {
                quote! {
                    if let Some(ref value) = self.#name {
                        writer.write_bits(1, 1)?; // Valid bit
                        writer.#write_fn(value, #byte_len)?;
                    } else {
                        writer.write_bits(0, 1)?; // Invalid bit
                        writer.#write_fn("", #byte_len)?; // Write empty padded string
                    }
                }
            } else {
                quote! { writer.#write_fn(&self.#name, #byte_len)?; }
            }
        }

        IRElement::Field { name, bits, string: None } => {
            let name = to_snake_case(name);
            if epb {
                quote! {
                    if let Some(value) = self.#name {
                        writer.write_bits(1, 1)?; // Valid bit
                        writer.write_bits(value as u64, #bits)?;
                    } else {
                        writer.write_bits(0, 1)?; // Invalid bit
                        writer.write_bits(0, #bits)?; // Zero value
                    }
                }
            } else {
                quote! { writer.write_bits(self.#name as u64, #bits)?; }
            }
        }

        IRElement::Enum { name, bits, .. } => {
            let name = to_snake_case(name);
            if epb {
                quote! {
                    if let Some(value) = self.#name {
                        writer.write_bits(1, 1)?; // Valid bit
                        writer.write_bits(u8::from(value) as u64, #bits)?;
                    } else {
                        writer.write_bits(0, 1)?; // Invalid bit
                        writer.write_bits(0, #bits)?; // Zero value
                    }
                }
            } else {
                quote! { writer.write_bits(u8::from(self.#name) as u64, #bits)?; }
            }
        }

        IRElement::EPB { .. } => unreachable!("nested EPB is rejected by IRLayout::validate"),
    }
}

/// Emits the writes for a run of elements, in wire order.
fn encode_elements(elements: &[IRElement]) -> Vec<TokenStream> {
    elements.iter().map(encode_element).collect()
}

/// Generates the encode implementation(s) for an item's layout.
pub fn generate_encode(name: &Ident, layout: &IRLayout) -> TokenStream {
    match layout {
        IRLayout::Fixed { elements, .. } => simple_encode(name, elements, None),
        IRLayout::Explicit { bytes, elements } => simple_encode(name, elements, Some(*bytes + 1)),
        IRLayout::Extended { part_groups, .. } => extended_encode(name, part_groups),
        IRLayout::Repetitive { counter_bits, elements, .. } => {
            repetitive_encode(name, *counter_bits, elements)
        }
        IRLayout::Compound { .. } => {
            unreachable!("compound items are assembled by item_gen")
        }
    }
}

/// Fixed and explicit items: write the elements straight through.
///
/// `total_bytes` is `Some` for an explicit item, whose payload is prefixed by
/// its own length counting the length byte itself.
fn simple_encode(name: &Ident, elements: &[IRElement], total_bytes: Option<usize>) -> TokenStream {
    let writes = encode_elements(elements);
    let length_byte = total_bytes.map(|total| {
        quote! { writer.write_bits(#total as u64, 8)?; }
    });

    quote! {
        impl Encode for #name {
            fn encode<W: std::io::Write>(
                &self,
                writer: &mut BitWriter<W>,
            ) -> Result<(), DecodeError> {
                #length_byte
                #(#writes)*
                Ok(())
            }
        }
    }
}

/// Extended items: each part followed by an FX bit saying whether another
/// part follows.
fn extended_encode(name: &Ident, part_groups: &[IRPartGroup]) -> TokenStream {
    let mut part_impls = Vec::new();
    let mut encode_body = Vec::new();
    let total_parts = part_groups.len();

    for (i, group) in part_groups.iter().enumerate() {
        let part_name = part_struct_name(name, group.index);
        let field_name = part_field_name(group.index);
        let writes = encode_elements(&group.elements);

        part_impls.push(quote! {
            impl #part_name {
                pub fn encode<W: std::io::Write>(
                    &self,
                    writer: &mut BitWriter<W>,
                ) -> Result<(), DecodeError> {
                    #(#writes)*
                    Ok(())
                }
            }
        });

        // The FX bit after a part reports whether the next one is present, so
        // it is read off the following field — except for the last part, which
        // always terminates the chain.
        let next_present = (i + 1 < total_parts).then(|| {
            let next_field = part_field_name(part_groups[i + 1].index);
            quote! { self.#next_field.is_some() as u64 }
        });
        let fx_bit = match &next_present {
            Some(next) => quote! { writer.write_bits(#next, 1)?; }, // FX bit
            None => quote! { writer.write_bits(0, 1)?; }, // FX bit = 0, no more extension
        };

        if i == 0 {
            encode_body.push(quote! {
                self.#field_name.encode(writer)?;
                #fx_bit
            });
        } else {
            encode_body.push(quote! {
                if let Some(ref part_data) = self.#field_name {
                    part_data.encode(writer)?;
                    #fx_bit
                }
            });
        }
    }

    quote! {
        #(#part_impls)*

        impl Encode for #name {
            fn encode<W: std::io::Write>(
                &self,
                writer: &mut BitWriter<W>,
            ) -> Result<(), DecodeError> {
                #(#encode_body)*
                Ok(())
            }
        }
    }
}

/// Repetitive items.
///
/// Wire format with counter: `[counter: counter_bits bits][element 0]...[element N-1]`
/// Wire format FX-terminated: `[element][FX]...` with FX = 1 while more follow
fn repetitive_encode(
    name: &Ident,
    counter_bits: Option<usize>,
    elements: &[IRElement],
) -> TokenStream {
    let element_name = element_struct_name(name);
    let writes = encode_elements(elements);

    let encode_body = match counter_bits {
        Some(counter_bits) => quote! {
            // Write the repetition counter
            writer.write_bits(self.items.len() as u64, #counter_bits)?;
            for item in &self.items {
                item.encode(writer)?;
            }
        },
        None => quote! {
            // FX-terminated: at least one repetition must be present, each
            // followed by an FX bit (1 = another repetition follows)
            if self.items.is_empty() {
                return Err(DecodeError::InvalidData(
                    "FX-terminated repetitive item requires at least one repetition",
                ));
            }
            let last = self.items.len() - 1;
            for (i, item) in self.items.iter().enumerate() {
                item.encode(writer)?;
                writer.write_bits((i != last) as u64, 1)?; // FX bit
            }
        },
    };

    quote! {
        impl #element_name {
            fn encode<W: std::io::Write>(
                &self,
                writer: &mut BitWriter<W>,
            ) -> Result<(), DecodeError> {
                #(#writes)*
                Ok(())
            }
        }

        impl Encode for #name {
            fn encode<W: std::io::Write>(
                &self,
                writer: &mut BitWriter<W>,
            ) -> Result<(), DecodeError> {
                #encode_body
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_utils::assert_code_contains;

    #[test]
    fn spare_bits_are_written_as_zero() {
        let layout = IRLayout::Fixed {
            bytes: 1,
            elements: vec![
                IRElement::Spare { bits: 3 },
                IRElement::Field { name: "code".into(), bits: 5, string: None },
            ],
        };

        let code = generate_encode(&format_ident!("Item010"), &layout).to_string();
        assert_code_contains(&code, &[
            "writer . write_bits (0 , 3usize) ? ;",
            "writer . write_bits (self . code as u64 , 5usize) ? ;",
        ]);
    }

    #[test]
    fn epb_writes_the_value_slot_even_when_absent() {
        // Decode always reads those bits, so encode has to emit them.
        let layout = IRLayout::Fixed {
            bytes: 2,
            elements: vec![IRElement::EPB {
                content: Box::new(IRElement::Field { name: "alt".into(), bits: 15, string: None }),
            }],
        };

        let code = generate_encode(&format_ident!("Item010"), &layout).to_string();
        assert_code_contains(&code, &[
            "if let Some (value) = self . alt",
            "writer . write_bits (1 , 1) ? ;",
            "writer . write_bits (0 , 1) ? ;",
            "writer . write_bits (0 , 15usize) ? ;",
        ]);
    }

    #[test]
    fn explicit_item_writes_a_length_that_counts_itself() {
        let layout = IRLayout::Explicit {
            bytes: 3,
            elements: vec![IRElement::Field { name: "code".into(), bits: 24, string: None }],
        };

        let code = generate_encode(&format_ident!("Item010"), &layout).to_string();
        assert!(code.contains("writer . write_bits (4usize as u64 , 8) ? ;"), "{code}");
    }

    #[test]
    fn extended_fx_bit_follows_the_next_part() {
        let part = |index: usize, name: &str| IRPartGroup {
            index,
            elements: vec![IRElement::Field { name: name.into(), bits: 7, string: None }],
        };
        let layout = IRLayout::Extended {
            bytes: 2,
            part_groups: vec![part(0, "a"), part(1, "b")],
        };

        let code = generate_encode(&format_ident!("Item030"), &layout).to_string();
        assert_code_contains(&code, &[
            "writer . write_bits (self . part1 . is_some () as u64 , 1) ? ;",
            "if let Some (ref part_data) = self . part1",
        ]);
    }
}
