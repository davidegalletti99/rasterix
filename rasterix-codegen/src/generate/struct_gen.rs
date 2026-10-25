use proc_macro2::{Ident, TokenStream};
use quote::quote;

use crate::transform::ir::{IRElement, IRLayout, IRPartGroup};
use super::utils::{
    data_fields, element_struct_name, field_type, part_field_name, part_struct_name,
};

/// Declares the struct(s) an item's layout needs.
pub fn generate_structs(name: &Ident, layout: &IRLayout) -> TokenStream {
    match layout {
        IRLayout::Fixed { elements, .. } | IRLayout::Explicit { elements, .. } => {
            generate_struct(name, elements)
        }
        IRLayout::Extended { part_groups, .. } => extended_structs(name, part_groups),
        IRLayout::Repetitive { elements, .. } => repetitive_structs(name, elements),
        IRLayout::Compound { .. } => {
            unreachable!("compound items are assembled by item_gen")
        }
    }
}

/// Declares a struct with one field per data-bearing element.
pub fn generate_struct(name: &Ident, elements: &[IRElement]) -> TokenStream {
    let fields = data_fields(elements);
    let field_tokens: Vec<_> = fields
        .iter()
        .map(|field| {
            let field_name = &field.name;
            let ty = field_type(field);
            quote! { pub #field_name: #ty }
        })
        .collect();

    quote! {
        #[derive(Debug, Clone, PartialEq)]
        pub struct #name {
            #(#field_tokens),*
        }
    }
}

/// Declares the repetition struct plus the container holding a `Vec` of it.
fn repetitive_structs(name: &Ident, elements: &[IRElement]) -> TokenStream {
    let element_name = element_struct_name(name);
    let element_struct = generate_struct(&element_name, elements);

    quote! {
        #element_struct

        #[derive(Debug, Clone, PartialEq)]
        pub struct #name {
            pub items: Vec<#element_name>,
        }
    }
}

/// Declares one struct per part plus the item that holds them.
fn extended_structs(name: &Ident, part_groups: &[IRPartGroup]) -> TokenStream {
    let mut part_structs = Vec::new();
    let mut fields = Vec::new();

    for group in part_groups {
        let part_name = part_struct_name(name, group.index);
        part_structs.push(generate_struct(&part_name, &group.elements));

        let field_name = part_field_name(group.index);
        // The first part is always on the wire; the rest arrive only when the
        // preceding FX bit is set.
        if group.index == 0 {
            fields.push(quote! { pub #field_name: #part_name });
        } else {
            fields.push(quote! { pub #field_name: Option<#part_name> });
        }
    }

    quote! {
        #(#part_structs)*

        #[derive(Debug, Clone, PartialEq)]
        pub struct #name {
            #(#fields),*
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transform::ir::StringKind;
    use quote::format_ident;
    use test_utils::assert_code_contains;

    fn elements() -> Vec<IRElement> {
        vec![
            IRElement::Field { name: "sac".into(), bits: 8, string: None },
            IRElement::Spare { bits: 4 },
            IRElement::Field { name: "alt".into(), bits: 12, string: None },
            IRElement::Field { name: "callsign".into(), bits: 48, string: Some(StringKind::Icao) },
            IRElement::EPB {
                content: Box::new(IRElement::Field { name: "speed".into(), bits: 16, string: None }),
            },
        ]
    }

    #[test]
    fn struct_fields_follow_element_types() {
        let code = generate_struct(&format_ident!("Item010"), &elements()).to_string();
        assert_code_contains(&code, &[
            "pub struct Item010",
            "pub sac : u8",
            "pub alt : u16",
            "pub callsign : String",
            "pub speed : Option < u16 >",
        ]);
        assert!(!code.contains("spare"), "spare bits must not become a field: {code}");
    }

    #[test]
    fn repetitive_layout_declares_element_and_container() {
        let layout = IRLayout::Repetitive {
            bytes: 1,
            counter_bits: Some(8),
            elements: vec![IRElement::Field { name: "code".into(), bits: 8, string: None }],
        };

        let code = generate_structs(&format_ident!("Item020"), &layout).to_string();
        assert_code_contains(&code, &[
            "pub struct Item020Element",
            "pub code : u8",
            "pub struct Item020",
            "pub items : Vec < Item020Element >",
        ]);
    }

    #[test]
    fn extended_layout_makes_only_the_first_part_required() {
        let part = |index: usize, name: &str| IRPartGroup {
            index,
            elements: vec![IRElement::Field { name: name.into(), bits: 7, string: None }],
        };
        let layout = IRLayout::Extended {
            bytes: 2,
            part_groups: vec![part(0, "a"), part(1, "b")],
        };

        let code = generate_structs(&format_ident!("Item030"), &layout).to_string();
        assert_code_contains(&code, &[
            "pub struct Item030Part0",
            "pub struct Item030Part1",
            "pub part0 : Item030Part0",
            "pub part1 : Option < Item030Part1 >",
        ]);
    }
}
