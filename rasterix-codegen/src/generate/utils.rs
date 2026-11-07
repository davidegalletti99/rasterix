use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};

use crate::transform::ir::{FieldKind, IRElement};

/// Maps a bit count to the Rust integer type that holds it, signed or not.
///
/// String fields do not go through this function — they become `String`.
///
/// # Examples
///
/// ```
/// use quote::format_ident;
/// use rasterix_codegen::generate::utils::rust_type_for_bits;
/// assert_eq!(rust_type_for_bits(3, false), format_ident!("u8"));
/// assert_eq!(rust_type_for_bits(12, true), format_ident!("i16"));
/// ```
pub fn rust_type_for_bits(bits: usize, signed: bool) -> Ident {
    let width = match bits {
        0..=8 => 8u8,
        9..=16 => 16,
        17..=32 => 32,
        33..=64 => 64,
        _ => 128,
    };
    format_ident!("{}{}", if signed { "i" } else { "u" }, width)
}

/// Converts a name to PascalCase for type names.
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
    format_ident!("{}", pascal_case(name))
}

/// The PascalCase spelling of `name`, which need not be an identifier on its
/// own (`"010"` stays `"010"`).
fn pascal_case(name: &str) -> String {
    let pascal_spaces_list= ['_', '-'];
    name
        .split(|c: char| pascal_spaces_list.contains(&c))
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
        .collect()
}

/// Converts a name to snake_case for field names.
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
    let chars: Vec<char> = name.chars().collect();
    let mut snake = String::with_capacity(name.len() + 4);

    for (i, &c) in chars.iter().enumerate() {
        if c.is_uppercase() && i > 0 {
            let prev_is_lower = chars[i - 1].is_lowercase();
            let next_is_lower = chars.get(i + 1).is_some_and(|n| n.is_lowercase());

            if prev_is_lower || next_is_lower {
                snake.push('_');
            }
        }
        snake.push(c.to_ascii_lowercase());
    }

    let snake = snake.replace('-', "_");
    // An identifier cannot start with a digit (asterix-specs' I048/110/3DH).
    if snake.starts_with(|c: char| c.is_ascii_digit()) {
        format_ident!("_{}", snake)
    } else {
        format_ident!("{}", snake)
    }
}

// ── Generated names ───────────────────────────────────────────────────────
//
// Several generators need the same name for the same thing: struct_gen
// declares a type, decode_gen and encode_gen implement on it. These functions
// are the single definition of each name, called wherever it is needed.

/// The record struct every category generates: `Record`.
pub fn record_struct_name() -> Ident {
    format_ident!("Record")
}

/// The module a category's code lands in: `cat048`.
pub fn module_name(category_id: u8) -> Ident {
    format_ident!("cat{category_id:03}")
}

/// An item's struct: `Item010`, `ItemSp`.
pub fn item_struct_name(id: &str) -> Ident {
    format_ident!("Item{}", pascal_case(id))
}

/// An item's field in the record: `item010`, `itemsp`.
pub fn item_field_name(id: &str) -> Ident {
    format_ident!("item{}", id.to_lowercase())
}

/// An enum's type, named after the struct declaring the field:
/// `Item020Part0Typ`. Enums are emitted at module level, so the field name
/// alone would collide across items.
pub fn enum_type_name(owner: &Ident, field: &str) -> Ident {
    format_ident!("{owner}{}", pascal_case(field))
}

/// One part of an extended item: `Item010Part0`.
pub fn part_struct_name(parent: &Ident, index: usize) -> Ident {
    format_ident!("{parent}Part{index}")
}

/// One part's field in its item: `part0`.
pub fn part_field_name(index: usize) -> Ident {
    format_ident!("part{index}")
}

/// A single repetition of a repetitive item: `Item020Element`.
pub fn element_struct_name(parent: &Ident) -> Ident {
    format_ident!("{parent}Element")
}

/// A compound sub-item's struct: `Item130Srl`.
pub fn sub_struct_name(parent: &Ident, name: &str) -> Ident {
    format_ident!("{parent}{}", pascal_case(name))
}

/// A compound sub-item's field in its parent: `srl`.
pub fn sub_field_name(name: &str) -> Ident {
    to_snake_case(name)
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
/// `frn` is **1-indexed** per EUROCONTROL-SPEC-0149. Returns
/// `(byte_index, bit_position)` for use with `Fspec::set()`; `bit_position` is
/// in range 0-6 and `Fspec` internally computes `1 << (7 - bit)`.
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

// ── Struct fields ─────────────────────────────────────────────────────────

/// A data-bearing element paired with the name it takes in a generated struct.
///
/// EPB wrappers are unwrapped: `inner` is the `Field` or `Enum` the EPB
/// contains, and `optional` records that its presence bit makes the struct
/// field an `Option`.
pub struct DataField<'a> {
    pub name: Ident,
    pub inner: &'a IRElement,
    pub optional: bool,
}

/// Unwraps an EPB to the element it wraps, reporting whether it was wrapped.
///
/// Every generator needs this pairing: the inner element says what to read or
/// write, the flag says whether a presence bit precedes it.
pub fn unwrap_epb(element: &IRElement) -> (&IRElement, bool) {
    match element {
        IRElement::EPB { content } => (content.as_ref(), true),
        other => (other, false),
    }
}

/// Pairs each data-bearing element with its field name, in declaration order.
///
/// Spares are skipped — they occupy wire bits but hold no data, so they do not
/// appear in the generated struct.
pub fn data_fields(elements: &[IRElement]) -> Vec<DataField<'_>> {
    elements
        .iter()
        .filter_map(|element| {
            let (inner, optional) = unwrap_epb(element);
            let name = match inner {
                IRElement::Field { name, .. } | IRElement::Enum { name, .. } => to_snake_case(name),
                // Spares hold no data; nested EPBs and EPB-wrapped spares are
                // rejected by IRLayout::validate before code generation.
                _ => return None,
            };
            Some(DataField { name, inner, optional })
        })
        .collect()
}

/// The Rust type a data field takes in `owner`, the generated struct
/// declaring it.
pub fn field_type(owner: &Ident, field: &DataField) -> TokenStream {
    let base = match field.inner {
        IRElement::Field { kind: FieldKind::String(_), .. } => quote! { String },
        IRElement::Field { bits, kind, .. } => {
            let ty = rust_type_for_bits(*bits, *kind == FieldKind::Signed);
            quote! { #ty }
        }
        IRElement::Enum { name, .. } => {
            let ty = enum_type_name(owner, name);
            quote! { #ty }
        }
        other => unreachable!("data_fields only yields Field or Enum, got {other:?}"),
    };

    if field.optional {
        quote! { Option<#base> }
    } else {
        base
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rust_type_for_bits() {
        assert_eq!(rust_type_for_bits(1, false), format_ident!("u8"));
        assert_eq!(rust_type_for_bits(8, false), format_ident!("u8"));
        assert_eq!(rust_type_for_bits(9, false), format_ident!("u16"));
        assert_eq!(rust_type_for_bits(16, false), format_ident!("u16"));
        assert_eq!(rust_type_for_bits(17, false), format_ident!("u32"));
        assert_eq!(rust_type_for_bits(32, false), format_ident!("u32"));
        assert_eq!(rust_type_for_bits(33, false), format_ident!("u64"));
        assert_eq!(rust_type_for_bits(64, false), format_ident!("u64"));
        assert_eq!(rust_type_for_bits(65, false), format_ident!("u128"));
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
        // asterix-specs names can start with a digit (I048/110/3DH).
        assert_eq!(to_snake_case("3DH"), format_ident!("_3dh"));
    }

    #[test]
    fn generated_names() {
        let item = item_struct_name("010");
        assert_eq!(item, format_ident!("Item010"));
        assert_eq!(item_field_name("010"), format_ident!("item010"));
        assert_eq!(item_struct_name("SP"), format_ident!("ItemSp"));
        assert_eq!(item_field_name("SP"), format_ident!("itemsp"));
        assert_eq!(
            enum_type_name(&format_ident!("Item030Element"), "value"),
            format_ident!("Item030ElementValue")
        );
        assert_eq!(module_name(48), format_ident!("cat048"));
        assert_eq!(part_struct_name(&item, 1), format_ident!("Item010Part1"));
        assert_eq!(element_struct_name(&item), format_ident!("Item010Element"));
        assert_eq!(sub_struct_name(&item, "sub2"), format_ident!("Item010Sub2"));
        assert_eq!(sub_struct_name(&item, "SRL"), format_ident!("Item010Srl"));
        assert_eq!(sub_field_name("SRL"), format_ident!("srl"));
    }

    #[test]
    fn data_fields_skips_spares_and_unwraps_epb() {
        let elements = vec![
            IRElement::Field { name: "sac".into(), bits: 8, kind: FieldKind::Unsigned },
            IRElement::Spare { bits: 4 },
            IRElement::EPB {
                content: Box::new(IRElement::Field { name: "alt".into(), bits: 12, kind: FieldKind::Unsigned }),
            },
        ];

        let fields = data_fields(&elements);
        let names: Vec<_> = fields.iter().map(|f| f.name.to_string()).collect();
        assert_eq!(names, ["sac", "alt"], "spare must not reach the struct");

        let owner = format_ident!("Item010");
        assert_eq!(field_type(&owner, &fields[0]).to_string(), "u8");
        assert_eq!(field_type(&owner, &fields[1]).to_string(), "Option < u16 >");
    }
}
