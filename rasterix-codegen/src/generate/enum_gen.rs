use proc_macro2::TokenStream;
use quote::quote;

use crate::transform::lower_ir::LoweredEnum;

/// Generates a Rust enum from a pre-lowered enum definition.
///
/// Creates an enum with:
/// - Named variants for all defined values
/// - An Unknown(u8) variant for undefined values
/// - TryFrom<u8> implementation for decoding
/// - Into<u8> implementation for encoding
pub fn generate_enum(lowered: &LoweredEnum) -> TokenStream {
    let enum_name = &lowered.name;

    let variants: Vec<_> = lowered.variants.iter().map(|v| {
        let vname = &v.name;
        let vval = v.value;
        quote! { #vname = #vval }
    }).collect();

    let try_from_arms: Vec<_> = lowered.variants.iter().map(|v| {
        let vname = &v.name;
        let vval = v.value;
        quote! { #vval => Ok(Self::#vname) }
    }).collect();

    let from_arms: Vec<_> = lowered.variants.iter().map(|v| {
        let vname = &v.name;
        let vval = v.value;
        quote! { #enum_name::#vname => #vval }
    }).collect();

    quote! {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        #[repr(u8)]
        pub enum #enum_name {
            #(#variants,)*
            Unknown(u8),
        }

        impl TryFrom<u8> for #enum_name {
            type Error = ();

            fn try_from(value: u8) -> Result<Self, ()> {
                match value {
                    #(#try_from_arms,)*
                    _ => Ok(Self::Unknown(value)),
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
    use quote::format_ident;
    use test_utils::assert_code_contains;
    use crate::transform::lower_ir::LoweredEnumVariant;

    #[test]
    fn test_generate_enum() {
        let lowered = LoweredEnum {
            name: format_ident!("TargetType"),
            variants: vec![
                LoweredEnumVariant { name: format_ident!("Psr"), value: 1 },
                LoweredEnumVariant { name: format_ident!("Ssr"), value: 2 },
                LoweredEnumVariant { name: format_ident!("Combined"), value: 3 },
            ],
        };

        let result = generate_enum(&lowered);
        let code = result.to_string();

        let expected_fragments = [
            "pub enum TargetType",
            "Psr = 1u8",
            "Ssr = 2u8",
            "Combined = 3u8",
            "Unknown (u8)",
            "impl TryFrom < u8 > for TargetType",
            "impl From < TargetType > for u8",
        ];
        assert_code_contains(&code, &expected_fragments);
    }
}