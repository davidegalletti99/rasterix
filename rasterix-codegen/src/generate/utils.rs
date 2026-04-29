use proc_macro2::Ident;
use quote::format_ident;

/// Maps a bit count to the appropriate Rust unsigned integer type.
///
/// String fields are handled separately via `FieldType::FixedString` in the
/// lowered IR and do not go through this function.
///
/// # Arguments
///
/// * `bits` - Number of bits needed
///
/// # Returns
///
/// The smallest unsigned integer type that can hold the specified number of bits.
///
/// # Examples
///
/// ```
/// use rasterix_codegen::generate::utils::rust_type_for_bits;
/// assert_eq!(rust_type_for_bits(3), "u8");
/// assert_eq!(rust_type_for_bits(12), "u16");
/// assert_eq!(rust_type_for_bits(24), "u32");
/// ```
pub fn rust_type_for_bits(bits: usize) -> String {
    match bits {
        0..=8 => "u8".to_string(),
        9..=16 => "u16".to_string(),
        17..=32 => "u32".to_string(),
        33..=64 => "u64".to_string(),
        _ => "u128".to_string()
    }
}

/// Converts a name to PascalCase for type names.
/// 
/// # Arguments
/// 
/// * `name` - The input name (can be snake_case, kebab-case, etc.)
/// 
/// # Returns
/// 
/// An Ident in PascalCase suitable for a Rust type name.
/// 
/// # Examples
/// 
/// ```
/// use quote::format_ident;
/// use rasterix_codegen::generate::utils::to_pascal_case;
/// assert_eq!(to_pascal_case("field_name"), format_ident!("FieldName"));
/// assert_eq!(to_pascal_case("SSR"), format_ident!("Ssr"));
/// ```
pub fn to_pascal_case(name: &str) -> Ident {
    let pascal = name
        .split(|c: char| c == '_' || c == '-')
        .filter(|s| !s.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => {
                    let rest: String = chars.collect();
                    format!("{}{}", first.to_uppercase(), rest.to_lowercase())
                }
            }
        })
        .collect::<String>();
    
    format_ident!("{}", pascal)
}

/// Converts a name to snake_case for field names.
/// 
/// # Arguments
/// 
/// * `name` - The input name
/// 
/// # Returns
/// 
/// An Ident in snake_case suitable for a Rust field name.
/// 
/// # Examples
/// 
/// ```
/// use quote::format_ident;
/// use rasterix_codegen::generate::utils::to_snake_case;
/// assert_eq!(to_snake_case("FieldName"), format_ident!("field_name"));
/// assert_eq!(to_snake_case("SSR"), format_ident!("ssr"));
/// ```
pub fn to_snake_case(name: &str) -> Ident {
    let snake = name
        .chars()
        .enumerate()
        .flat_map(|(i, c)| {
            if i > 0 {
                let prev = name.chars().nth(i - 1);
                let next = name.chars().nth(i + 1);
                if c.is_uppercase() && 
                ((prev.is_some() && prev.unwrap().is_lowercase()) || 
                (next.is_some() && next.unwrap().is_lowercase())) {
                    vec!['_', c.to_ascii_lowercase()]
                } else {
                    
                    vec![c.to_ascii_lowercase()]
                }

            } else {
                vec![c.to_ascii_lowercase()]
            }
        })
        .collect::<String>()
        .replace('-', "_");
    
    format_ident!("{}", snake)
}

/// Generates a unique type name for a nested structure.
/// 
/// # Arguments
/// 
/// * `parent_name` - The name of the parent item/struct
/// * `suffix` - A descriptive suffix (e.g., "Byte0", "Sub1")
/// 
/// # Returns
/// 
/// A unique type name combining the parent and suffix.
/// 
/// # Examples
///
/// ```
/// use quote::format_ident;
/// use rasterix_codegen::generate::utils::nested_type_name;
/// assert_eq!(
///     nested_type_name("Item020", "Byte0"),
///     format_ident!("Item020Byte0")
/// );
/// ```
#[allow(unused)]
pub fn nested_type_name(parent_name: &str, suffix: &str) -> Ident {
    format_ident!("{}{}", parent_name, suffix)
}

/// Calculates the FSPEC byte and bit position from an FRN.
///
/// ASTERIX FSPEC layout (each byte has 7 data bits + 1 FX bit):
/// - FRN 1  → byte 0, bit 7 (0x80)  ← first item per ASTERIX standard
/// - FRN 2  → byte 0, bit 6 (0x40)
/// - FRN 7  → byte 0, bit 1 (0x02)
/// - (bit 0 is the FX continuation bit, not used for items)
/// - FRN 8  → byte 1, bit 7 (0x80)
/// - FRN 14 → byte 1, bit 1 (0x02)
///
/// # Arguments
///
/// * `frn` - The Field Reference Number, **1-indexed** per the ASTERIX standard
///           (EUROCONTROL-SPEC-0149). FRN 1 is the first item in the FSPEC,
///           mapping to the MSB of the first FSPEC byte.
///
/// # Returns
///
/// A tuple of `(byte_index, bit_position)` for use with `Fspec::set()`.
/// `bit_position` is in range 0–6; `Fspec` internally computes `1 << (7 - bit)`.
///
/// # Panics
///
/// Panics if `frn` is 0. FRN is 1-indexed per the ASTERIX standard.
pub fn frn_to_fspec_position(frn: usize) -> (usize, u8) {
    assert!(frn >= 1, "FRN must be >= 1 (1-indexed per ASTERIX standard), got {frn}");
    let zero = frn - 1;
    let byte = zero / 7;
    let bit  = zero % 7;
    (byte, bit as u8)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_rust_type_for_bits() {
        assert_eq!(rust_type_for_bits(1), "u8");
        assert_eq!(rust_type_for_bits(8), "u8");
        assert_eq!(rust_type_for_bits(9), "u16");
        assert_eq!(rust_type_for_bits(16), "u16");
        assert_eq!(rust_type_for_bits(17), "u32");
        assert_eq!(rust_type_for_bits(32), "u32");
        assert_eq!(rust_type_for_bits(33), "u64");
        assert_eq!(rust_type_for_bits(64), "u64");
        assert_eq!(rust_type_for_bits(65), "u128");
    }

    #[test]
    fn test_frn_to_fspec_position() {
        // FRN 1-7 map to byte 0, bits 0-6 (Fspec computes 1 << (7-bit))
        assert_eq!(frn_to_fspec_position(1), (0, 0)); // → 0x80
        assert_eq!(frn_to_fspec_position(2), (0, 1)); // → 0x40
        assert_eq!(frn_to_fspec_position(7), (0, 6)); // → 0x02
        // FRN 8-14 map to byte 1
        assert_eq!(frn_to_fspec_position(8), (1, 0)); // → 0x80 in byte 1
        assert_eq!(frn_to_fspec_position(14), (1, 6)); // → 0x02 in byte 1
        // FRN 15+ map to byte 2
        assert_eq!(frn_to_fspec_position(15), (2, 0)); // → 0x80 in byte 2
    }
    
    #[test]
    fn test_to_pascal_case() {
        assert_eq!(to_pascal_case("test"), format_ident!("Test"));
        assert_eq!(to_pascal_case("field_name"), format_ident!("FieldName"));
        assert_eq!(to_pascal_case("SSR"), format_ident!("Ssr"));
        assert_eq!(to_pascal_case("mode_3a"), format_ident!("Mode3a"));
    }
    
    #[test]
    fn test_to_snake_case() {
        assert_eq!(to_snake_case("Test"), format_ident!("test"));
        assert_eq!(to_snake_case("FieldName"), format_ident!("field_name"));
        assert_eq!(to_snake_case("SSR"), format_ident!("ssr"));
    }
}