//! Transforms an asterix-specs definition into the intermediate representation.

use crate::error::CodegenError;
use crate::parse::spec_model::*;
use crate::transform::ir::{
    FieldKind, IRElement, IRItem, IRLayout, IRPartGroup, IRSubItem, StringKind, UapSlot, IR,
};

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
pub(crate) fn flatten<'a>(
    path: &str,
    prefix: &str,
    items: impl IntoIterator<Item = &'a Item>,
) -> Result<Vec<IRElement>, CodegenError> {
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
        let variation = context_free(&format!("{path}/{name}"), &non_spare.rule)?;
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

/// Transforms an asterix-specs category definition into the IR.
///
/// # Errors
///
/// Returns [`CodegenError::Unsupported`] for constructs rasterix cannot
/// generate yet (expansions, multiple UAPs, `rfs`, `case`, BDS, ...), and the
/// [`IR::new`] validation errors otherwise.
pub fn spec_to_ir(spec: Asterix) -> Result<IR, CodegenError> {
    let basic = match spec {
        Asterix::AsterixBasic(basic) => basic,
        Asterix::AsterixExpansion(_) => return Err(unsupported("file", "expansion (REF) definitions")),
    };
    let cat = format!("I{:03}", basic.category);

    let items = basic
        .catalogue
        .iter()
        .map(|item| {
            let path = format!("{cat}/{}", item.name);
            Ok(IRItem { id: item.name.clone(), layout: layout(&path, context_free(&path, &item.rule)?)? })
        })
        .collect::<Result<Vec<_>, CodegenError>>()?;

    let uap = match basic.uap {
        Uap::Uap(slots) => slots
            .into_iter()
            .map(|slot| match slot {
                UapItem::UapItem(name) => Ok(UapSlot::Item(name)),
                UapItem::UapItemSpare(_) => Ok(UapSlot::Spare),
                UapItem::UapItemRfs(_) => Err(unsupported(&format!("{cat}/UAP"), "random field sequencing (rfs)")),
            })
            .collect::<Result<Vec<_>, _>>()?,
        Uap::Uaps(_) => return Err(unsupported(&format!("{cat}/UAP"), "multiple UAPs")),
    };

    IR::new(basic.category, items, uap)
}

fn context_free<'a>(path: &str, rule: &'a Rule<Variation>) -> Result<&'a Variation, CodegenError> {
    match rule {
        Rule::ContextFree(variation) => Ok(variation),
        Rule::Dependent(_) => Err(unsupported(path, "a \"Dependent\" rule (case)")),
    }
}

/// Total width of `elements`, which must fill whole bytes.
fn whole_bytes(path: &str, bits: usize) -> Result<usize, CodegenError> {
    if bits.is_multiple_of(8) {
        Ok(bits / 8)
    } else {
        Err(unsupported(path, &format!("a {bits}-bit structure that does not fill whole bytes")))
    }
}

fn bit_count(elements: &[IRElement]) -> usize {
    elements.iter().map(IRElement::bit_size).sum()
}

/// The elements of a structure that is a single element (one field named
/// `value`) or a group.
fn simple_elements(path: &str, variation: &Variation) -> Result<Vec<IRElement>, CodegenError> {
    match variation {
        Variation::Element { bit_size, rule } => Ok(vec![element(path, "value", *bit_size, rule)?]),
        Variation::Group(items) => flatten(path, "", items),
        _ => Err(unsupported(path, "a repetition of a non-group structure")),
    }
}

/// Maps the structure of the item or sub-item at `path`.
fn layout(path: &str, variation: &Variation) -> Result<IRLayout, CodegenError> {
    Ok(match variation {
        Variation::Element { .. } | Variation::Group(_) => {
            let elements = simple_elements(path, variation)?;
            IRLayout::Fixed { bytes: whole_bytes(path, bit_count(&elements))?, elements }
        }

        Variation::Extended(items) => {
            // Each None is an FX bit closing a part; the last part must be
            // closed too, or the item could never be extended past it.
            if !matches!(items.last(), Some(None)) {
                return Err(unsupported(path, "an extended item whose last part has no FX bit"));
            }
            let parts: Vec<&[Option<Item>]> = items[..items.len() - 1].split(Option::is_none).collect();
            let part_groups = parts
                .iter()
                .enumerate()
                .map(|(index, part)| Ok(IRPartGroup { index, elements: flatten(path, "", part.iter().flatten())? }))
                .collect::<Result<Vec<_>, CodegenError>>()?;
            IRLayout::Extended { bytes: part_groups.len(), part_groups }
        }

        Variation::Repetitive { kind, variation } => {
            let elements = simple_elements(path, variation)?;
            let bits = bit_count(&elements);
            match kind {
                RepetitiveType::RepetitiveRegular { byte_size } => IRLayout::Repetitive {
                    bytes: whole_bytes(path, bits)?,
                    counter_bits: Some(8 * byte_size),
                    elements,
                },
                // Each repetition ends in an FX bit.
                RepetitiveType::RepetitiveFx(_) => IRLayout::Repetitive {
                    bytes: whole_bytes(path, bits + 1)?,
                    counter_bits: None,
                    elements,
                },
            }
        }

        Variation::Explicit(_) => IRLayout::OpaqueExplicit,

        Variation::Compound(subs) => {
            let mut sub_items = Vec::new();
            for (i, sub) in subs.iter().enumerate() {
                // None is an unassigned FSPEC slot.
                let Some(sub) = sub else { continue };
                let sub_path = format!("{path}/{}", sub.name);
                sub_items.push(IRSubItem {
                    index: i + 1,
                    name: sub.name.clone(),
                    layout: layout(&sub_path, context_free(&sub_path, &sub.rule)?)?,
                });
            }
            IRLayout::Compound { sub_items }
        }
    })
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

    // ── Layouts, UAP, category ───────────────────────────────────────────

    use crate::transform::ir::{IRLayout, UapSlot, IR};

    fn top(name: &str, variation: Variation) -> NonSpare {
        NonSpare { name: name.into(), rule: Rule::ContextFree(variation) }
    }

    /// A CAT048 definition holding `catalogue`, with every item in the UAP.
    fn category(catalogue: Vec<NonSpare>) -> Asterix {
        let uap = catalogue.iter().map(|item| UapItem::UapItem(item.name.clone())).collect();
        Asterix::AsterixBasic(Basic { category: 48, catalogue, uap: Uap::Uap(uap) })
    }

    fn layout_of(variation: Variation) -> IRLayout {
        let ir: IR = spec_to_ir(category(vec![top("010", variation)])).unwrap();
        ir.category.items.into_iter().next().unwrap().layout
    }

    fn unsupported_feature(spec: Asterix) -> (String, String) {
        match spec_to_ir(spec).unwrap_err() {
            CodegenError::Unsupported { path, feature } => (path, feature),
            other => panic!("expected Unsupported, got {other}"),
        }
    }

    fn raw_element(bits: usize) -> Variation {
        context_free_element(bits, Content::ContentRaw([]))
    }

    #[test]
    fn element_item_is_one_value_field() {
        let IRLayout::Fixed { bytes: 3, elements } = layout_of(raw_element(24)) else { panic!("not fixed") };
        assert_eq!(field_names(&elements), ["value"]);
    }

    #[test]
    fn group_item_is_fixed() {
        let IRLayout::Fixed { bytes: 2, elements } = layout_of(Variation::Group(vec![raw("SAC", 8), raw("SIC", 8)])) else {
            panic!("not fixed")
        };
        assert_eq!(field_names(&elements), ["sac", "sic"]);
    }

    #[test]
    fn element_item_not_byte_aligned_is_unsupported() {
        let (path, _) = unsupported_feature(category(vec![top("010", raw_element(7))]));
        assert_eq!(path, "I048/010");
    }

    #[test]
    fn extended_splits_parts_at_fx() {
        let items = vec![
            Some(raw("TYP", 3)), Some(raw("SIM", 1)), Some(raw("RDP", 1)), Some(raw("SPI", 1)), Some(raw("RAB", 1)),
            None,
            Some(raw("TST", 1)), Some(Item::Spare(6)),
            None,
        ];

        let IRLayout::Extended { bytes: 2, part_groups } = layout_of(Variation::Extended(items)) else { panic!("not extended") };
        assert_eq!(part_groups.iter().map(|g| g.index).collect::<Vec<_>>(), [0, 1]);
        assert_eq!(field_names(&part_groups[0].elements), ["typ", "sim", "rdp", "spi", "rab"]);
        assert_eq!(field_names(&part_groups[1].elements), ["tst"]);
    }

    #[test]
    fn extended_without_final_fx_is_unsupported() {
        let items = vec![Some(raw("A", 7)), None, Some(raw("B", 8))];
        let (path, _) = unsupported_feature(category(vec![top("271", Variation::Extended(items))]));
        assert_eq!(path, "I048/271");
    }

    #[test]
    fn regular_repetitive_counts_in_bytes() {
        let group = Variation::Group(vec![raw("DOP", 16), raw("AMB", 16), raw("FRQ", 16)]);
        let rep = Variation::Repetitive {
            kind: RepetitiveType::RepetitiveRegular { byte_size: 1 },
            variation: Box::new(group),
        };

        let IRLayout::Repetitive { bytes: 6, counter_bits: Some(8), elements } = layout_of(rep) else { panic!("wrong layout") };
        assert_eq!(field_names(&elements), ["dop", "amb", "frq"]);
    }

    #[test]
    fn fx_repetitive_of_an_element_has_one_value_field() {
        let rep = Variation::Repetitive { kind: RepetitiveType::RepetitiveFx([]), variation: Box::new(raw_element(7)) };

        let IRLayout::Repetitive { bytes: 1, counter_bits: None, elements } = layout_of(rep) else { panic!("wrong layout") };
        assert_eq!(field_names(&elements), ["value"]);
    }

    #[test]
    fn explicit_is_opaque() {
        assert!(matches!(layout_of(Variation::Explicit(serde::de::IgnoredAny)), IRLayout::OpaqueExplicit));
    }

    #[test]
    fn compound_sub_items_keep_their_fspec_position_and_name() {
        let subs = vec![Some(top("SRL", raw_element(8))), None, Some(top("SAM", raw_element(8)))];

        let IRLayout::Compound { sub_items } = layout_of(Variation::Compound(subs)) else { panic!("not compound") };
        let slots: Vec<(usize, &str)> = sub_items.iter().map(|s| (s.index, s.name.as_str())).collect();
        assert_eq!(slots, [(1, "SRL"), (3, "SAM")]);
    }

    #[test]
    fn dependent_item_is_unsupported() {
        let item = NonSpare { name: "380".into(), rule: Rule::Dependent(serde::de::IgnoredAny) };
        let (path, _) = unsupported_feature(category(vec![item]));
        assert_eq!(path, "I048/380");
    }

    #[test]
    fn expansion_is_unsupported() {
        let (_, feature) = unsupported_feature(Asterix::AsterixExpansion(serde::de::IgnoredAny));
        assert!(feature.contains("expansion"), "{feature}");
    }

    #[test]
    fn uaps_is_unsupported() {
        let spec = Asterix::AsterixBasic(Basic {
            category: 1,
            catalogue: vec![top("010", raw_element(8))],
            uap: Uap::Uaps(serde::de::IgnoredAny),
        });
        let (_, feature) = unsupported_feature(spec);
        assert!(feature.contains("multiple UAPs"), "{feature}");
    }

    #[test]
    fn rfs_slot_is_unsupported() {
        let spec = Asterix::AsterixBasic(Basic {
            category: 1,
            catalogue: vec![top("010", raw_element(8))],
            uap: Uap::Uap(vec![UapItem::UapItem("010".into()), UapItem::UapItemRfs([])]),
        });
        let (_, feature) = unsupported_feature(spec);
        assert!(feature.contains("rfs"), "{feature}");
    }

    #[test]
    fn category_and_uap_reach_the_ir() {
        let spec = Asterix::AsterixBasic(Basic {
            category: 48,
            catalogue: vec![top("010", raw_element(8)), top("020", raw_element(8))],
            uap: Uap::Uap(vec![
                UapItem::UapItem("010".into()),
                UapItem::UapItemSpare([]),
                UapItem::UapItem("020".into()),
            ]),
        });

        let ir = spec_to_ir(spec).unwrap();
        assert_eq!(ir.category.id, 48);
        assert_eq!(ir.category.uap, [UapSlot::Item("010".into()), UapSlot::Spare, UapSlot::Item("020".into())]);
    }
}
