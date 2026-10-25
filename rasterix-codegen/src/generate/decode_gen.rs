use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};

use crate::transform::ir::{IRElement, IRLayout, IRPartGroup, StringKind};
use super::utils::{
    data_fields, element_struct_name, part_field_name, part_struct_name, rust_type_for_bits,
    to_pascal_case, to_snake_case, unwrap_epb,
};

/// Returns the BitReader method used to read a string of the given kind.
fn read_string_fn(kind: StringKind) -> Ident {
    match kind {
        StringKind::Icao => format_ident!("read_string"),
        StringKind::Ascii => format_ident!("read_ascii_string"),
    }
}

/// Emits the read for one element, binding it to a local named after the
/// struct field it fills.
///
/// An EPB-wrapped element reads its presence bit and then its value either
/// way: the value's bits are on the wire whether or not it is valid, so the
/// read cannot be skipped, only its result discarded.
fn decode_element(element: &IRElement) -> TokenStream {
    let (inner, epb) = unwrap_epb(element);

    let (name, read) = match inner {
        IRElement::Spare { bits } => {
            debug_assert!(!epb, "EPB-wrapped spare is rejected by IRLayout::validate");
            // Spare bits carry no data; consume them and bind nothing.
            return quote! { reader.read_bits(#bits)?; };
        }
        IRElement::Field { name, bits, string: Some(kind) } => {
            let read_fn = read_string_fn(*kind);
            let byte_len = bits / 8;
            (to_snake_case(name), quote! { reader.#read_fn(#byte_len)? })
        }
        IRElement::Field { name, bits, string: None } => {
            let rust_type = rust_type_for_bits(*bits);
            (to_snake_case(name), quote! { reader.read_bits(#bits)? as #rust_type })
        }
        IRElement::Enum { name, bits, .. } => {
            let enum_type = to_pascal_case(name);
            (
                to_snake_case(name),
                quote! { #enum_type::from(reader.read_bits(#bits)? as u8) },
            )
        }
        IRElement::EPB { .. } => unreachable!("nested EPB is rejected by IRLayout::validate"),
    };

    if epb {
        quote! {
            let #name = {
                let valid = reader.read_bits(1)? != 0;
                let value = #read;
                if valid { Some(value) } else { None }
            };
        }
    } else {
        quote! { let #name = #read; }
    }
}

/// Emits the reads for a run of elements, in wire order.
fn decode_elements(elements: &[IRElement]) -> Vec<TokenStream> {
    elements.iter().map(decode_element).collect()
}

/// The struct's field names, in declaration order, for `Ok(Self { .. })`.
fn field_names(elements: &[IRElement]) -> Vec<Ident> {
    data_fields(elements).into_iter().map(|field| field.name).collect()
}

/// Generates the decode implementation(s) for an item's layout.
pub fn generate_decode(name: &Ident, layout: &IRLayout) -> TokenStream {
    match layout {
        IRLayout::Fixed { elements, .. } => simple_decode(name, elements, false),
        IRLayout::Explicit { elements, .. } => simple_decode(name, elements, true),
        IRLayout::Extended { part_groups, .. } => extended_decode(name, part_groups),
        IRLayout::Repetitive { counter_bits, elements, .. } => {
            repetitive_decode(name, *counter_bits, elements)
        }
        IRLayout::Compound { .. } => {
            unreachable!("compound items are assembled by item_gen")
        }
    }
}

/// Fixed and explicit items: read the elements straight through.
fn simple_decode(name: &Ident, elements: &[IRElement], explicit: bool) -> TokenStream {
    let reads = decode_elements(elements);
    let fields = field_names(elements);

    // An explicit item is prefixed by its own length. The elements already
    // describe the payload, so the byte is read and dropped.
    let length_byte = explicit.then(|| {
        quote! {
            // Length includes itself, so actual data is len - 1 bytes
            let _len = reader.read_bits(8)? as usize;
        }
    });

    quote! {
        impl Decode for #name {
            fn decode<R: std::io::Read>(
                reader: &mut BitReader<R>,
            ) -> Result<Self, DecodeError> {
                #length_byte
                #(#reads)*

                Ok(Self {
                    #(#fields),*
                })
            }
        }
    }
}

/// Extended items: one struct per part, chained by FX bits.
fn extended_decode(name: &Ident, part_groups: &[IRPartGroup]) -> TokenStream {
    let mut part_impls = Vec::new();
    let mut decode_body = Vec::new();
    let mut item_fields = Vec::new();

    let number_of_parts = part_groups.len();
    for (i, group) in part_groups.iter().enumerate() {
        let part_name = part_struct_name(name, group.index);
        let field_name = part_field_name(group.index);

        let reads = decode_elements(&group.elements);
        let part_fields = field_names(&group.elements);

        part_impls.push(quote! {
            impl #part_name {
                pub fn decode<R: std::io::Read>(
                    reader: &mut BitReader<R>,
                ) -> Result<Self, DecodeError> {
                    #(#reads)*
                    Ok(Self { #(#part_fields),* })
                }
            }
        });

        if i == 0 {
            decode_body.push(quote! {
                let #field_name = #part_name::decode(reader)?;
            });

            if i != number_of_parts - 1 {
                if number_of_parts <= 2 {
                    // no need for mutable fx if there are only 2 parts
                    decode_body.push(quote! {
                        let fx = reader.read_bits(1)? != 0;
                    });
                } else {
                    // For more than 2 parts, we need to keep track of fx for
                    // subsequent parts
                    decode_body.push(quote! {
                        let mut fx = reader.read_bits(1)? != 0;
                    });
                }
            } else {
                // Single-part item: the encoder still terminates it with an
                // FX bit, so it has to be consumed here.
                decode_body.push(quote! {
                    reader.read_bits(1)?;
                });
            }
        } else if i != number_of_parts - 1 {
            decode_body.push(quote! {
                let #field_name = if fx {
                    let part = #part_name::decode(reader)?;
                    fx = reader.read_bits(1)? != 0;
                    Some(part)
                } else {
                    None
                };
            });
        } else {
            decode_body.push(quote! {
                let #field_name = if fx {
                    let part = #part_name::decode(reader)?;
                    // Last part carries a terminating FX bit on the wire.
                    reader.read_bits(1)?;
                    Some(part)
                } else {
                    None
                };
            });
        }

        item_fields.push(part_field_name(group.index));
    }

    quote! {
        #(#part_impls)*

        impl Decode for #name {
            fn decode<R: std::io::Read>(
                reader: &mut BitReader<R>,
            ) -> Result<Self, DecodeError> {
                #(#decode_body)*

                Ok(Self {
                    #(#item_fields),*
                })
            }
        }
    }
}

/// Repetitive items.
///
/// Wire format with counter: `[counter: counter_bits bits][element 0]...[element N-1]`
/// Wire format FX-terminated: `[element][FX]...` with FX = 1 while more follow
fn repetitive_decode(
    name: &Ident,
    counter_bits: Option<usize>,
    elements: &[IRElement],
) -> TokenStream {
    let element_name = element_struct_name(name);
    let reads = decode_elements(elements);
    let fields = field_names(elements);

    let decode_body = match counter_bits {
        Some(counter_bits) => quote! {
            // Read the repetition counter
            let count = reader.read_bits(#counter_bits)? as usize;
            let mut items = Vec::with_capacity(count);
            for _ in 0..count {
                items.push(#element_name::decode(reader)?);
            }
        },
        None => quote! {
            // FX-terminated: each repetition ends with an FX bit,
            // 1 = another repetition follows
            let mut items = Vec::new();
            loop {
                items.push(#element_name::decode(reader)?);
                if reader.read_bits(1)? == 0 {
                    break;
                }
            }
        },
    };

    quote! {
        impl #element_name {
            fn decode<R: std::io::Read>(
                reader: &mut BitReader<R>,
            ) -> Result<Self, DecodeError> {
                #(#reads)*

                Ok(Self {
                    #(#fields),*
                })
            }
        }

        impl Decode for #name {
            fn decode<R: std::io::Read>(
                reader: &mut BitReader<R>,
            ) -> Result<Self, DecodeError> {
                #decode_body

                Ok(Self { items })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_utils::assert_code_contains;

    #[test]
    fn spare_bits_are_consumed_without_a_field() {
        let elements = vec![
            IRElement::Spare { bits: 3 },
            IRElement::Field { name: "code".into(), bits: 5, string: None },
        ];
        let layout = IRLayout::Fixed { bytes: 1, elements };

        let code = generate_decode(&format_ident!("Item010"), &layout).to_string();
        assert_code_contains(&code, &[
            "reader . read_bits (3usize) ? ;",
            "let code = reader . read_bits (5usize) ? as u8 ;",
        ]);
    }

    #[test]
    fn epb_reads_its_value_whether_or_not_it_is_valid() {
        // The value's bits are on the wire either way; skipping the read would
        // leave the reader out of phase.
        let elements = vec![IRElement::EPB {
            content: Box::new(IRElement::Field { name: "alt".into(), bits: 15, string: None }),
        }];
        let layout = IRLayout::Fixed { bytes: 2, elements };

        let code = generate_decode(&format_ident!("Item010"), &layout).to_string();
        assert_code_contains(&code, &[
            "let valid = reader . read_bits (1) ? != 0 ;",
            "let value = reader . read_bits (15usize) ? as u16 ;",
            "if valid { Some (value) } else { None }",
        ]);
    }

    #[test]
    fn explicit_item_reads_its_length_byte() {
        let layout = IRLayout::Explicit {
            bytes: 1,
            elements: vec![IRElement::Field { name: "code".into(), bits: 8, string: None }],
        };

        let code = generate_decode(&format_ident!("Item010"), &layout).to_string();
        assert!(code.contains("let _len = reader . read_bits (8) ? as usize ;"), "{code}");
    }

    #[test]
    fn fx_terminated_repetitive_loops_until_the_bit_clears() {
        let layout = IRLayout::Repetitive {
            bytes: 1,
            counter_bits: None,
            elements: vec![IRElement::Field { name: "code".into(), bits: 7, string: None }],
        };

        let code = generate_decode(&format_ident!("Item020"), &layout).to_string();
        assert_code_contains(&code, &[
            "impl Item020Element",
            "loop",
            "if reader . read_bits (1) ? == 0 { break",
        ]);
        assert!(!code.contains("with_capacity"), "no counter means no known count: {code}");
    }
}
