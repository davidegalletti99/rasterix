use proc_macro2::{Ident, TokenStream};
use quote::quote;

use crate::transform::ir::{IRElement, IRLayout};
use super::utils::{element_struct_name, enum_type_name, part_struct_name, to_pascal_case, unwrap_epb};

/// An enum declared by some element, ready to be emitted at module level.
pub struct EnumDef<'a> {
    pub type_name: Ident,
    pub values: &'a [(String, u8)],
}

/// Collects every enum declared directly by a layout's elements.
///
/// Generated enums live at module level, not inside the struct that uses them,
/// so they have to be hoisted out of the elements declaring them. Compound
/// sub-items are not descended into — item_gen emits each sub-item's enums
/// alongside the sub-item itself.
///
/// Each enum is named after the struct that declares its field (see
/// [`enum_type_name`]): the item `name` itself, its repetition struct, or one
/// of its parts.
pub fn collect_enums<'a>(name: &Ident, layout: &'a IRLayout) -> Vec<EnumDef<'a>> {
    let element_groups: Vec<(Ident, &[IRElement])> = match layout {
        IRLayout::Fixed { elements, .. } | IRLayout::Explicit { elements, .. } => {
            vec![(name.clone(), elements)]
        }
        IRLayout::Repetitive { elements, .. } => vec![(element_struct_name(name), elements)],
        IRLayout::Extended { part_groups, .. } => part_groups
            .iter()
            .map(|g| (part_struct_name(name, g.index), g.elements.as_slice()))
            .collect(),
        IRLayout::OpaqueExplicit | IRLayout::Compound { .. } => vec![],
    };

    element_groups
        .into_iter()
        .flat_map(|(owner, elements)| {
            elements.iter().filter_map(move |element| match unwrap_epb(element).0 {
                IRElement::Enum { name, values, .. } => {
                    Some(EnumDef { type_name: enum_type_name(&owner, name), values })
                }
                _ => None,
            })
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
    let enum_name = &def.type_name;

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
    use quote::format_ident;
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
        let IRElement::Enum { values, .. } = &element else { unreachable!() };

        let def = EnumDef { type_name: format_ident!("Item010TargetType"), values };
        let code = generate_enum(&def).to_string();
        assert_code_contains(&code, &[
            "pub enum Item010TargetType",
            "Psr = 1u8",
            "Ssr = 2u8",
            "Combined = 3u8",
            "Unknown (u8)",
            "impl From < u8 > for Item010TargetType",
            "impl From < Item010TargetType > for u8",
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

        let names: Vec<String> = collect_enums(&format_ident!("Item020"), &layout)
            .iter().map(|e| e.type_name.to_string()).collect();
        assert_eq!(names, ["Item020Part0TargetType", "Item020Part1TargetType"]);
    }

    #[test]
    fn enum_types_are_prefixed_by_their_owning_struct() {
        let enum_el = |name: &str| IRElement::Enum { name: name.into(), bits: 7, values: vec![("a".into(), 0)] };
        let name = format_ident!("Item030");

        let fixed = IRLayout::Fixed { bytes: 1, elements: vec![enum_el("typ")] };
        let repetitive = IRLayout::Repetitive { bytes: 1, counter_bits: None, elements: vec![enum_el("value")] };
        let type_names = |layout: &IRLayout| -> Vec<String> {
            collect_enums(&name, layout).iter().map(|e| e.type_name.to_string()).collect()
        };

        assert_eq!(type_names(&fixed), ["Item030Typ"]);
        assert_eq!(type_names(&repetitive), ["Item030ElementValue"]);
    }
}
