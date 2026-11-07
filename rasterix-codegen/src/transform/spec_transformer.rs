//! Transforms an asterix-specs definition into the intermediate representation.

use crate::error::CodegenError;
use crate::parse::spec_model::*;
use crate::transform::ir::{FieldKind, IRElement, StringKind};

fn unsupported(path: &str, feature: &str) -> CodegenError {
    CodegenError::Unsupported { path: path.to_string(), feature: feature.to_string() }
}

/// Derives a variant name for each table entry from its description.
///
/// Names are snake_case; the generator turns them into PascalCase variants.
/// The text is split on every non-alphanumeric character. A name that would
/// be empty or start with a digit gets a `v_` prefix, and a name that is
/// `unknown` (taken by the generated `Unknown(u8)` fallback) or appears more
/// than once in the table gets the entry's value appended.
pub(crate) fn variant_names(table: &[(u64, String)]) -> Vec<String> {
    let base: Vec<String> = table
        .iter()
        .map(|(_, text)| {
            let name = text
                .split(|c: char| !c.is_ascii_alphanumeric())
                .filter(|word| !word.is_empty())
                .map(str::to_ascii_lowercase)
                .collect::<Vec<_>>()
                .join("_");
            if name.is_empty() || name.starts_with(|c: char| c.is_ascii_digit()) {
                format!("v_{name}")
            } else {
                name
            }
        })
        .collect();

    base.iter()
        .zip(table)
        .map(|(name, (value, _))| {
            let clashes = name == "unknown" || base.iter().filter(|other| *other == name).count() > 1;
            if clashes { format!("{name}_{value}") } else { name.clone() }
        })
        .collect()
}

/// Maps one element to a field or enum named after `name`, lowercased.
/// `path` is the enclosing item's path, e.g. `I048/020`.
pub(crate) fn element(path: &str, name: &str, bits: usize, rule: &Rule<Content>) -> Result<IRElement, CodegenError> {
    let path = format!("{path}/{name}");
    let field = |kind| IRElement::Field { name: name.to_ascii_lowercase(), bits, kind };
    let signed = |signedness: &Signedness| match signedness {
        Signedness::Signed(_) => FieldKind::Signed,
        Signedness::Unsigned(_) => FieldKind::Unsigned,
    };

    let content = match rule {
        Rule::ContextFree(content) => content,
        Rule::Dependent(_) => return Err(unsupported(&path, "a \"Dependent\" rule (case)")),
    };
    Ok(match content {
        Content::ContentRaw(_) => field(FieldKind::Unsigned),
        Content::ContentInteger { signedness } | Content::ContentQuantity { signedness } => field(signed(signedness)),
        Content::ContentString(StringType::StringIcao(_)) => field(FieldKind::String(StringKind::Icao)),
        Content::ContentString(StringType::StringAscii(_)) => field(FieldKind::String(StringKind::Ascii)),
        // Octal is only how the value is displayed; the bits are a plain number.
        Content::ContentString(StringType::StringOctal(_)) => field(FieldKind::Unsigned),
        Content::ContentTable(table) => {
            if bits > 8 {
                return Err(unsupported(&path, "a table on an element wider than 8 bits"));
            }
            let values = variant_names(table)
                .into_iter()
                .zip(table)
                .map(|(variant, (value, _))| (variant, *value as u8))
                .collect();
            IRElement::Enum { name: name.to_ascii_lowercase(), bits, values }
        }
        Content::ContentBds(_) => return Err(unsupported(&path, "BDS register content")),
    })
}

/// Flattens a group's items into IR elements, in wire order.
///
/// Leaves of a nested group are prefixed with the nested group names (`ADSB`
/// → `adsb_ep`), since sibling groups reuse leaf names; leaves directly under
/// the item keep their own name. `prefix` is the prefix accumulated so far.
pub(crate) fn flatten(path: &str, prefix: &str, items: &[Item]) -> Result<Vec<IRElement>, CodegenError> {
    let mut elements = Vec::new();
    for item in items {
        let non_spare = match item {
            Item::Spare(bits) => {
                elements.push(IRElement::Spare { bits: *bits });
                continue;
            }
            Item::Item(non_spare) => non_spare,
        };
        let name = &non_spare.name;
        let variation = match &non_spare.rule {
            Rule::ContextFree(variation) => variation,
            Rule::Dependent(_) => return Err(unsupported(&format!("{path}/{name}"), "a \"Dependent\" rule (case)")),
        };
        match variation {
            Variation::Element { bit_size, rule } => {
                let prefixed = format!("{prefix}{name}");
                let mut el = element(path, name, *bit_size, rule)?;
                if let IRElement::Field { name, .. } | IRElement::Enum { name, .. } = &mut el {
                    *name = prefixed.to_ascii_lowercase();
                }
                elements.push(el);
            }
            Variation::Group(nested) => {
                let nested_prefix = format!("{prefix}{}_", name.to_ascii_lowercase());
                elements.extend(flatten(&format!("{path}/{name}"), &nested_prefix, nested)?);
            }
            _ => return Err(unsupported(&format!("{path}/{name}"), "a non-group structure inside a group")),
        }
    }
    Ok(elements)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context_free_element(bits: usize, content: Content) -> Variation {
        Variation::Element { bit_size: bits, rule: Rule::ContextFree(content) }
    }

    fn named(name: &str, variation: Variation) -> Item {
        Item::Item(NonSpare { name: name.into(), rule: Rule::ContextFree(variation) })
    }

    fn raw(name: &str, bits: usize) -> Item {
        named(name, context_free_element(bits, Content::ContentRaw([])))
    }

    fn table(entries: &[(u64, &str)]) -> Content {
        Content::ContentTable(entries.iter().map(|(v, s)| (*v, s.to_string())).collect())
    }

    fn field_names(elements: &[IRElement]) -> Vec<&str> {
        elements
            .iter()
            .filter_map(|e| match e {
                IRElement::Field { name, .. } | IRElement::Enum { name, .. } => Some(name.as_str()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn table_text_becomes_variant_names() {
        let t = |v: u64, s: &str| (v, s.to_string());
        assert_eq!(
            variant_names(&[t(0, "No detection"), t(7, "ModeS Roll-Call +PSR"), t(2, "1 bit")]),
            ["no_detection", "modes_roll_call_psr", "v_1_bit"]
        );
    }

    #[test]
    fn unknown_and_duplicate_texts_get_their_value() {
        let t = |v: u64, s: &str| (v, s.to_string());
        assert_eq!(
            variant_names(&[t(0, "Not defined"), t(1, "Not-defined"), t(3, "Unknown")]),
            ["not_defined_0", "not_defined_1", "unknown_3"]
        );
    }

    #[test]
    fn empty_table_text_gets_a_name() {
        assert_eq!(variant_names(&[(4, "--".to_string())]), ["v_"]);
    }

    #[test]
    fn nested_group_leaves_are_prefixed() {
        let adsb = named("ADSB", Variation::Group(vec![raw("EP", 1), raw("VAL", 1)]));
        let items = vec![raw("TYP", 3), adsb, Item::Spare(3)];

        let elements = flatten("I048/020", "", &items).unwrap();
        assert_eq!(field_names(&elements), ["typ", "adsb_ep", "adsb_val"]);
        assert!(matches!(elements.last(), Some(IRElement::Spare { bits: 3 })));
    }

    #[test]
    fn content_kinds_map_to_field_kinds() {
        let kind_of = |content: Content| match element("I048/010", "X", 8, &Rule::ContextFree(content)).unwrap() {
            IRElement::Field { kind, bits: 8, ref name } if name == "x" => kind,
            other => panic!("expected a field, got {other:?}"),
        };
        let signed = || Signedness::Signed([]);
        let unsigned = || Signedness::Unsigned([]);

        assert_eq!(kind_of(Content::ContentRaw([])), FieldKind::Unsigned);
        assert_eq!(kind_of(Content::ContentInteger { signedness: unsigned() }), FieldKind::Unsigned);
        assert_eq!(kind_of(Content::ContentInteger { signedness: signed() }), FieldKind::Signed);
        assert_eq!(kind_of(Content::ContentQuantity { signedness: unsigned() }), FieldKind::Unsigned);
        assert_eq!(kind_of(Content::ContentQuantity { signedness: signed() }), FieldKind::Signed);
        assert_eq!(
            kind_of(Content::ContentString(StringType::StringIcao([]))),
            FieldKind::String(StringKind::Icao)
        );
        assert_eq!(
            kind_of(Content::ContentString(StringType::StringAscii([]))),
            FieldKind::String(StringKind::Ascii)
        );
        assert_eq!(kind_of(Content::ContentString(StringType::StringOctal([]))), FieldKind::Unsigned);
    }

    #[test]
    fn table_becomes_an_enum() {
        let rule = Rule::ContextFree(table(&[(0, "No detection"), (1, "Single PSR detection")]));

        let el = element("I048/020", "TYP", 3, &rule).unwrap();
        let IRElement::Enum { name, bits: 3, values } = el else { panic!("expected an enum, got {el:?}") };
        assert_eq!(name, "typ");
        assert_eq!(values, [("no_detection".to_string(), 0), ("single_psr_detection".to_string(), 1)]);
    }

    #[test]
    fn wide_table_is_unsupported() {
        let rule = Rule::ContextFree(table(&[(0, "a")]));

        let err = element("I048/020", "X", 9, &rule).unwrap_err();
        assert!(matches!(err, CodegenError::Unsupported { ref path, .. } if path == "I048/020/X"), "got: {err}");
    }

    #[test]
    fn dependent_rule_is_unsupported() {
        let rule = Rule::Dependent(serde::de::IgnoredAny);

        let err = element("I062/380", "IAS", 15, &rule).unwrap_err();
        assert!(matches!(err, CodegenError::Unsupported { ref path, .. } if path == "I062/380/IAS"), "got: {err}");
    }

    #[test]
    fn bds_is_unsupported() {
        let rule = Rule::ContextFree(Content::ContentBds(serde::de::IgnoredAny));

        let err = element("I048/250", "MBDATA", 56, &rule).unwrap_err();
        assert!(matches!(err, CodegenError::Unsupported { .. }), "got: {err}");
    }

    #[test]
    fn non_group_variation_inside_a_group_is_unsupported() {
        let rep = named(
            "R",
            Variation::Repetitive {
                kind: RepetitiveType::RepetitiveFx([]),
                variation: Box::new(context_free_element(7, Content::ContentRaw([]))),
            },
        );

        let err = flatten("I048/020", "", &[rep]).unwrap_err();
        assert!(matches!(err, CodegenError::Unsupported { ref path, .. } if path == "I048/020/R"), "got: {err}");
    }
}
