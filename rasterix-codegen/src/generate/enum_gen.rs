use proc_macro2::TokenStream;
use quote::quote;

use crate::transform::ir::{IRElement, IRLayout};
use super::utils::{to_pascal_case, unwrap_epb};

/// An enum declared by some element, ready to be emitted at module level.
pub struct EnumDef<'a> {
    pub name: &'a str,
    pub values: &'a [(String, u8)],
}

/// Collects every enum declared directly by a layout's elements.
///
/// Generated enums live at module level, not inside the struct that uses them,
/// so they have to be hoisted out of the elements declaring them. Compound
/// sub-items are not descended into — item_gen emits each sub-item's enums
/// alongside the sub-item itself.
pub fn collect_enums(layout: &IRLayout) -> Vec<EnumDef<'_>> {
    let element_groups: Vec<&[IRElement]> = match layout {
        IRLayout::Fixed { elements, .. }
        | IRLayout::Explicit { elements, .. }
        | IRLayout::Repetitive { elements, .. } => vec![elements],
        IRLayout::Extended { part_groups, .. } => {
            part_groups.iter().map(|g| g.elements.as_slice()).collect()
        }
        IRLayout::Compound { .. } => vec![],
    };

    element_groups
        .into_iter()
        .flatten()
        .filter_map(|element| match unwrap_epb(element).0 {
            IRElement::Enum { name, values, .. } => Some(EnumDef { name, values }),
            _ => None,
        })
        .collect()
}

/// Generates a Rust enum from an enum declaration.
///
/// Creates an enum with:
/// - Named variants for all defined values
/// - An Unknown(u8) variant for undefined values
/// - From<u8> implementation for decoding (undefined values map to Unknown)
/// - Into<u8> implementation for encoding
pub fn generate_enum(def: &EnumDef) -> TokenStream {
    let enum_name = to_pascal_case(def.name);

    let variants: Vec<_> = def.values.iter().map(|(name, value)| {
        let vname = to_pascal_case(name);
        quote! { #vname = #value }
    }).collect();

    let from_u8_arms: Vec<_> = def.values.iter().map(|(name, value)| {
        let vname = to_pascal_case(name);
        quote! { #value => Self::#vname }
    }).collect();

    let from_arms: Vec<_> = def.values.iter().map(|(name, value)| {
        let vname = to_pascal_case(name);
        quote! { #enum_name::#vname => #value }
    }).collect();

    quote! {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        #[repr(u8)]
        pub enum #enum_name {
            #(#variants,)*
            Unknown(u8),
        }

        impl From<u8> for #enum_name {
            fn from(value: u8) -> Self {
                match value {
                    #(#from_u8_arms,)*
                    _ => Self::Unknown(value),
                }
            }
        }

        impl From<#enum_name> for u8 {
            fn from(val: #enum_name) -> u8 {
                match val {
                    #(#from_arms,)*
                    #enum_name::Unknown(v) => v,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transform::ir::IRPartGroup;
    use test_utils::assert_code_contains;

    fn target_type() -> IRElement {
        IRElement::Enum {
            name: "target_type".into(),
            bits: 2,
            values: vec![("psr".into(), 1), ("ssr".into(), 2), ("combined".into(), 3)],
        }
    }

    #[test]
    fn test_generate_enum() {
        let element = target_type();
        let IRElement::Enum { name, values, .. } = &element else { unreachable!() };

        let code = generate_enum(&EnumDef { name, values }).to_string();
        assert_code_contains(&code, &[
            "pub enum TargetType",
            "Psr = 1u8",
            "Ssr = 2u8",
            "Combined = 3u8",
            "Unknown (u8)",
            "impl From < u8 > for TargetType",
            "impl From < TargetType > for u8",
        ]);
    }

    #[test]
    fn enums_are_collected_from_every_part_and_through_epb() {
        let layout = IRLayout::Extended {
            bytes: 2,
            part_groups: vec![
                IRPartGroup { index: 0, elements: vec![target_type()] },
                IRPartGroup {
                    index: 1,
                    elements: vec![IRElement::EPB { content: Box::new(target_type()) }],
                },
            ],
        };

        let names: Vec<_> = collect_enums(&layout).iter().map(|e| e.name).collect();
        assert_eq!(names, ["target_type", "target_type"]);
    }
}
