use proc_macro2::{Ident, TokenStream};
use quote::quote;

use crate::transform::lower_ir::{FieldDescriptor, FieldType, LoweredPart};

/// Generates a struct field declaration from a pre-resolved field descriptor.
fn generate_field(field: &FieldDescriptor) -> TokenStream {
    let name = &field.name;
    match &field.type_tokens {
        FieldType::Primitive(ty) => quote! { pub #name: #ty },
        FieldType::OptionalPrimitive(ty) => quote! { pub #name: Option<#ty> },
        FieldType::Enum(ty) => quote! { pub #name: #ty },
        FieldType::OptionalEnum(ty) => quote! { pub #name: Option<#ty> },
        FieldType::FixedString(_) => quote! { pub #name: String },
        FieldType::OptionalFixedString(_) => quote! { pub #name: Option<String> },
    }
}

/// Generates a complete struct definition from flat field descriptors.
pub fn generate_struct(name: &Ident, fields: &[FieldDescriptor]) -> TokenStream {
    let field_tokens: Vec<_> = fields.iter().map(generate_field).collect();

    quote! {
        #[derive(Debug, Clone, PartialEq)]
        pub struct #name {
            #(#field_tokens),*
        }
    }
}

/// Generates a repetitive struct (element struct + container with Vec).
pub fn generate_repetitive_struct(
    name: &Ident,
    element_type_name: &Ident,
    fields: &[FieldDescriptor],
) -> TokenStream {
    let element_struct = generate_struct(element_type_name, fields);

    quote! {
        #element_struct

        #[derive(Debug, Clone, PartialEq)]
        pub struct #name {
            pub items: Vec<#element_type_name>,
        }
    }
}

/// Generates structs for an extended item from lowered parts.
pub fn generate_extended_structs(
    name: &Ident,
    parts: &[LoweredPart],
) -> TokenStream {
    let mut all_structs = Vec::new();
    let mut main_fields = Vec::new();

    for part in parts {
        let part_struct = generate_struct(&part.struct_name, &part.fields);
        all_structs.push(part_struct);

        let field_name = &part.field_name;
        let part_name = &part.struct_name;

        if part.is_required {
            main_fields.push(quote! { pub #field_name: #part_name });
        } else {
            main_fields.push(quote! { pub #field_name: Option<#part_name> });
        }
    }

    quote! {
        #(#all_structs)*

        #[derive(Debug, Clone, PartialEq)]
        pub struct #name {
            #(#main_fields),*
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use quote::format_ident;

    #[test]
    fn test_generate_field_primitive() {
        let field = FieldDescriptor {
            name: format_ident!("test_field"),
            type_tokens: FieldType::Primitive(format_ident!("u8")),
        };

        let result = generate_field(&field);
        let code = result.to_string();
        assert!(code.contains("pub test_field : u8"));
    }

    #[test]
    fn test_generate_field_optional() {
        let field = FieldDescriptor {
            name: format_ident!("optional_field"),
            type_tokens: FieldType::OptionalPrimitive(format_ident!("u16")),
        };

        let result = generate_field(&field);
        let code = result.to_string();
        assert!(code.contains("pub optional_field : Option < u16 >"));
    }

    #[test]
    fn test_generate_field_fixed_string() {
        let field = FieldDescriptor {
            name: format_ident!("aircraft_id"),
            type_tokens: FieldType::FixedString(6),
        };

        let result = generate_field(&field);
        let code = result.to_string();
        assert!(code.contains("pub aircraft_id : String"));
    }

    #[test]
    fn test_generate_field_optional_fixed_string() {
        let field = FieldDescriptor {
            name: format_ident!("callsign"),
            type_tokens: FieldType::OptionalFixedString(8),
        };

        let result = generate_field(&field);
        let code = result.to_string();
        assert!(code.contains("pub callsign : Option < String >"));
    }

    #[test]
    fn test_generate_struct() {
        let fields = vec![
            FieldDescriptor {
                name: format_ident!("sac"),
                type_tokens: FieldType::Primitive(format_ident!("u8")),
            },
            FieldDescriptor {
                name: format_ident!("sic"),
                type_tokens: FieldType::Primitive(format_ident!("u8")),
            },
        ];

        let result = generate_struct(&format_ident!("Item010"), &fields);
        let code = result.to_string();
        assert!(code.contains("pub struct Item010"));
        assert!(code.contains("pub sac : u8"));
        assert!(code.contains("pub sic : u8"));
    }
}
