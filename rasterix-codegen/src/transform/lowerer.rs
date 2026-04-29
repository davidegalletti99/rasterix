use proc_macro2::Ident;
use quote::format_ident;

use crate::generate::utils::{frn_to_fspec_position, rust_type_for_bits, to_pascal_case, to_snake_case};
use super::ir::*;
use super::lower_ir::*;

/// Lowers the semantic IR into a flat, code-generation-oriented representation.
pub fn lower(ir: &IR) -> LoweredIR {
    let category = &ir.category;

    LoweredIR {
        category_id: category.id,
        module_name: format_ident!("cat{:03}", category.id),
        record: lower_record(category),
        items: category.items.iter().map(lower_item).collect(),
    }
}

fn lower_record(category: &IRCategory) -> LoweredRecord {
    let entries = category.items.iter().map(|item| {
        let (fspec_byte, fspec_bit) = frn_to_fspec_position(item.frn as usize);
        RecordEntry {
            field_name: format_ident!("item{:03}", item.id),
            type_name: format_ident!("Item{:03}", item.id),
            fspec_byte,
            fspec_bit,
        }
    }).collect();

    LoweredRecord {
        name: format_ident!("Record"),
        entries,
    }
}

fn lower_item(item: &IRItem) -> LoweredItem {
    let name = format_ident!("Item{:03}", item.id);
    let enums = collect_and_lower_enums(&item.layout);
    let kind = lower_layout(&name, &item.layout);

    LoweredItem { name, enums, kind }
}

fn lower_layout(parent_name: &Ident, layout: &IRLayout) -> LoweredItemKind {
    match layout {
        IRLayout::Fixed { bytes, elements } => {
            LoweredItemKind::Simple {
                is_explicit: false,
                byte_size: *bytes,
                fields: lower_fields(elements),
                decode_ops: lower_decode_ops(elements, false),
                encode_ops: lower_encode_ops(elements, false, *bytes),
            }
        }
        IRLayout::Explicit { bytes, elements } => {
            LoweredItemKind::Simple {
                is_explicit: true,
                byte_size: *bytes,
                fields: lower_fields(elements),
                decode_ops: lower_decode_ops(elements, true),
                encode_ops: lower_encode_ops(elements, true, *bytes),
            }
        }
        IRLayout::Extended { part_groups, .. } => {
            let parts = part_groups.iter().map(|group| {
                LoweredPart {
                    index: group.index,
                    struct_name: format_ident!("{}Part{}", parent_name, group.index),
                    field_name: format_ident!("part{}", group.index),
                    is_required: group.index == 0,
                    fields: lower_fields(&group.elements),
                    decode_ops: lower_element_ops_decode(&group.elements),
                    encode_ops: lower_element_ops_encode(&group.elements),
                }
            }).collect();
            LoweredItemKind::Extended { parts }
        }
        IRLayout::Repetitive { bytes: _, counter_bytes, elements } => {
            let element_type_name = format_ident!("{}Element", parent_name);
            LoweredItemKind::Repetitive {
                element_type_name,
                counter_bytes: *counter_bytes,
                fields: lower_fields(elements),
                decode_ops: lower_element_ops_decode(elements),
                encode_ops: lower_element_ops_encode(elements),
            }
        }
        IRLayout::Compound { sub_items } => {
            let lowered_subs = sub_items.iter().map(|sub| {
                assert!(
                    !matches!(sub.layout, IRLayout::Compound { .. }),
                    "Nested compounds are not supported"
                );
                let sub_name = format_ident!("{}Sub{}", parent_name, sub.index);
                let (fspec_byte, fspec_bit) = frn_to_fspec_position(sub.index);
                let enums = collect_and_lower_enums(&sub.layout);
                let kind = lower_layout(&sub_name, &sub.layout);
                LoweredSubItem {
                    index: sub.index,
                    struct_name: sub_name,
                    field_name: format_ident!("sub{}", sub.index),
                    fspec_byte,
                    fspec_bit,
                    enums,
                    kind,
                }
            }).collect();
            LoweredItemKind::Compound { sub_items: lowered_subs }
        }
    }
}

// ── Field Lowering ────────────────────────────────────────────────────────

fn lower_fields(elements: &[IRElement]) -> Vec<FieldDescriptor> {
    elements.iter().filter_map(lower_field).collect()
}

fn lower_field(element: &IRElement) -> Option<FieldDescriptor> {
    match element {
        IRElement::EPB { content } => field_descriptor_for(content, true),
        other => field_descriptor_for(other, false),
    }
}

fn field_descriptor_for(element: &IRElement, optional: bool) -> Option<FieldDescriptor> {
    match element {
        IRElement::Field { name, bits, is_string } => {
            let field_name = to_snake_case(name);
            if *is_string {
                let byte_len = bits / 8;
                let type_tokens = if optional { FieldType::OptionalFixedString(byte_len) } else { FieldType::FixedString(byte_len) };
                Some(FieldDescriptor { name: field_name, type_tokens })
            } else {
                let rust_type = format_ident!("{}", rust_type_for_bits(*bits));
                let type_tokens = if optional { FieldType::OptionalPrimitive(rust_type) } else { FieldType::Primitive(rust_type) };
                Some(FieldDescriptor { name: field_name, type_tokens })
            }
        }
        IRElement::Enum { name, .. } => {
            let field_name = to_snake_case(name);
            let enum_type = to_pascal_case(name);
            let type_tokens = if optional { FieldType::OptionalEnum(enum_type) } else { FieldType::Enum(enum_type) };
            Some(FieldDescriptor { name: field_name, type_tokens })
        }
        IRElement::Spare { .. } => None,
        IRElement::EPB { .. } => panic!("Nested EPB not supported"),
    }
}

// ── Decode Op Lowering ────────────────────────────────────────────────────

fn lower_decode_ops(elements: &[IRElement], is_explicit: bool) -> Vec<DecodeOp> {
    let mut ops = Vec::new();
    if is_explicit {
        ops.push(DecodeOp::ReadLengthByte);
    }
    ops.extend(lower_element_ops_decode(elements));
    ops
}

fn lower_element_ops_decode(elements: &[IRElement]) -> Vec<DecodeOp> {
    elements.iter().map(lower_element_decode).collect()
}

fn lower_element_decode(element: &IRElement) -> DecodeOp {
    lower_element_decode_inner(element, false)
}

fn lower_element_decode_inner(element: &IRElement, is_epb: bool) -> DecodeOp {
    match element {
        IRElement::Field { name, bits, is_string } => {
            let name = to_snake_case(name);
            if *is_string {
                let byte_len = bits / 8;
                if is_epb { DecodeOp::ReadEpbString { name, byte_len } }
                else { DecodeOp::ReadString { name, byte_len } }
            } else {
                let rust_type = format_ident!("{}", rust_type_for_bits(*bits));
                if is_epb { DecodeOp::ReadEpbField { name, bits: *bits, rust_type } }
                else { DecodeOp::ReadField { name, bits: *bits, rust_type } }
            }
        }
        IRElement::EPB { content } => lower_element_decode_inner(content, true),
        IRElement::Enum { name, bits, .. } => {
            let name = to_snake_case(name);
            let enum_type = to_pascal_case(name.to_string().as_str());
            if is_epb { DecodeOp::ReadEpbEnum { name, bits: *bits, enum_type } }
            else { DecodeOp::ReadEnum { name, bits: *bits, enum_type } }
        }
        IRElement::Spare { bits } => {
            assert!(!is_epb, "EPB can only contain Field or Enum");
            DecodeOp::SkipSpare { bits: *bits }
        }
    }
}

// ── Encode Op Lowering ────────────────────────────────────────────────────

fn lower_encode_ops(elements: &[IRElement], is_explicit: bool, byte_size: usize) -> Vec<EncodeOp> {
    let mut ops = Vec::new();
    if is_explicit {
        ops.push(EncodeOp::WriteLengthByte { total_bytes: byte_size + 1 });
    }
    ops.extend(lower_element_ops_encode(elements));
    ops
}

fn lower_element_ops_encode(elements: &[IRElement]) -> Vec<EncodeOp> {
    elements.iter().map(lower_element_encode).collect()
}

fn lower_element_encode(element: &IRElement) -> EncodeOp {
    lower_element_encode_inner(element, false)
}

fn lower_element_encode_inner(element: &IRElement, is_epb: bool) -> EncodeOp {
    match element {
        IRElement::Field { name, bits, is_string } => {
            let name = to_snake_case(name);
            if *is_string {
                let byte_len = bits / 8;
                if is_epb { EncodeOp::WriteEpbString { name, byte_len } }
                else { EncodeOp::WriteString { name, byte_len } }
            } else {
                if is_epb { EncodeOp::WriteEpbField { name, bits: *bits } }
                else { EncodeOp::WriteField { name, bits: *bits } }
            }
        }
        IRElement::EPB { content } => lower_element_encode_inner(content, true),
        IRElement::Enum { name, bits, .. } => {
            let name = to_snake_case(name);
            if is_epb { EncodeOp::WriteEpbEnum { name, bits: *bits } }
            else { EncodeOp::WriteEnum { name, bits: *bits } }
        }
        IRElement::Spare { bits } => {
            assert!(!is_epb, "EPB can only contain Field or Enum");
            EncodeOp::WriteSpare { bits: *bits }
        }
    }
}

// ── Enum Collection ───────────────────────────────────────────────────────

fn collect_and_lower_enums(layout: &IRLayout) -> Vec<LoweredEnum> {
    let mut enums = Vec::new();
    match layout {
        IRLayout::Fixed { elements, .. } | IRLayout::Explicit { elements, .. } => {
            collect_enums_from_elements(elements, &mut enums);
        }
        IRLayout::Extended { part_groups, .. } => {
            for group in part_groups {
                collect_enums_from_elements(&group.elements, &mut enums);
            }
        }
        IRLayout::Repetitive { elements, .. } => {
            collect_enums_from_elements(elements, &mut enums);
        }
        IRLayout::Compound { sub_items } => {
            for sub_item in sub_items {
                enums.extend(collect_and_lower_enums(&sub_item.layout));
            }
        }
    }
    enums
}

fn collect_enums_from_elements(elements: &[IRElement], enums: &mut Vec<LoweredEnum>) {
    for element in elements {
        match element {
            IRElement::Enum { name, bits: _, values } => {
                enums.push(lower_enum(name, values));
            }
            IRElement::EPB { content } => {
                if let IRElement::Enum { name, bits: _, values } = content.as_ref() {
                    enums.push(lower_enum(name, values));
                }
            }
            _ => {}
        }
    }
}

fn lower_enum(name: &str, values: &[(String, u8)]) -> LoweredEnum {
    LoweredEnum {
        name: to_pascal_case(name),
        variants: values.iter().map(|(vname, vval)| {
            LoweredEnumVariant {
                name: to_pascal_case(vname),
                value: *vval,
            }
        }).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lower_fixed_item() {
        let ir = IR {
            category: IRCategory {
                id: 48,
                items: vec![IRItem {
                    id: 10,
                    frn: 1,
                    layout: IRLayout::Fixed {
                        bytes: 2,
                        elements: vec![
                            IRElement::Field { name: "sac".to_string(), bits: 8 , is_string: false},
                            IRElement::Field { name: "sic".to_string(), bits: 8, is_string: false},
                        ],
                    },
                }],
            },
        };

        let lowered = lower(&ir);
        assert_eq!(lowered.category_id, 48);
        assert_eq!(lowered.module_name, format_ident!("cat048"));
        assert_eq!(lowered.items.len(), 1);

        let item = &lowered.items[0];
        assert_eq!(item.name, format_ident!("Item010"));

        match &item.kind {
            LoweredItemKind::Simple { is_explicit, fields, decode_ops, encode_ops, .. } => {
                assert!(!is_explicit);
                assert_eq!(fields.len(), 2);
                assert_eq!(fields[0].name, format_ident!("sac"));
                assert_eq!(fields[1].name, format_ident!("sic"));
                assert_eq!(decode_ops.len(), 2);
                assert_eq!(encode_ops.len(), 2);
            }
            _ => panic!("Expected Simple kind"),
        }
    }

    #[test]
    fn test_lower_explicit_item() {
        let ir = IR {
            category: IRCategory {
                id: 48,
                items: vec![IRItem {
                    id: 20,
                    frn: 2,
                    layout: IRLayout::Explicit {
                        bytes: 2,
                        elements: vec![
                            IRElement::Field { name: "data".to_string(), bits: 16, is_string: false },
                        ],
                    },
                }],
            },
        };

        let lowered = lower(&ir);
        let item = &lowered.items[0];

        match &item.kind {
            LoweredItemKind::Simple { is_explicit, decode_ops, encode_ops, .. } => {
                assert!(is_explicit);
                assert!(matches!(decode_ops[0], DecodeOp::ReadLengthByte));
                assert!(matches!(encode_ops[0], EncodeOp::WriteLengthByte { total_bytes: 3 }));
            }
            _ => panic!("Expected Simple kind"),
        }
    }

    #[test]
    fn test_lower_spare_filtered_from_fields() {
        let ir = IR {
            category: IRCategory {
                id: 48,
                items: vec![IRItem {
                    id: 20,
                    frn: 2,
                    layout: IRLayout::Fixed {
                        bytes: 1,
                        elements: vec![
                            IRElement::Field { name: "data".to_string(), bits: 3, is_string: false },
                            IRElement::Spare { bits: 5 },
                        ],
                    },
                }],
            },
        };

        let lowered = lower(&ir);
        let item = &lowered.items[0];

        match &item.kind {
            LoweredItemKind::Simple { fields, decode_ops, encode_ops, .. } => {
                assert_eq!(fields.len(), 1);
                assert_eq!(decode_ops.len(), 2);
                assert_eq!(encode_ops.len(), 2);
                assert!(matches!(decode_ops[1], DecodeOp::SkipSpare { bits: 5 }));
                assert!(matches!(encode_ops[1], EncodeOp::WriteSpare { bits: 5 }));
            }
            _ => panic!("Expected Simple kind"),
        }
    }

    #[test]
    fn test_lower_epb_element() {
        let ir = IR {
            category: IRCategory {
                id: 48,
                items: vec![IRItem {
                    id: 30,
                    frn: 3,
                    layout: IRLayout::Fixed {
                        bytes: 2,
                        elements: vec![
                            IRElement::EPB {
                                content: Box::new(IRElement::Field {
                                    name: "opt_val".to_string(),
                                    bits: 15,
                                    is_string: false,
                                }),
                            },
                        ],
                    },
                }],
            },
        };

        let lowered = lower(&ir);
        let item = &lowered.items[0];

        match &item.kind {
            LoweredItemKind::Simple { fields, decode_ops, .. } => {
                assert_eq!(fields.len(), 1);
                assert!(matches!(fields[0].type_tokens, FieldType::OptionalPrimitive(_)));
                assert!(matches!(decode_ops[0], DecodeOp::ReadEpbField { .. }));
            }
            _ => panic!("Expected Simple kind"),
        }
    }

    #[test]
    fn test_lower_enum_collected() {
        let ir = IR {
            category: IRCategory {
                id: 48,
                items: vec![IRItem {
                    id: 20,
                    frn: 2,
                    layout: IRLayout::Fixed {
                        bytes: 1,
                        elements: vec![
                            IRElement::Enum {
                                name: "target_type".to_string(),
                                bits: 3,
                                values: vec![
                                    ("PSR".to_string(), 1),
                                    ("SSR".to_string(), 2),
                                ],
                            },
                            IRElement::Spare { bits: 5 },
                        ],
                    },
                }],
            },
        };

        let lowered = lower(&ir);
        let item = &lowered.items[0];

        assert_eq!(item.enums.len(), 1);
        assert_eq!(item.enums[0].name, format_ident!("TargetType"));
        assert_eq!(item.enums[0].variants.len(), 2);
        assert_eq!(item.enums[0].variants[0].name, format_ident!("Psr"));
        assert_eq!(item.enums[0].variants[0].value, 1);
    }

    #[test]
    fn test_lower_record_fspec() {
        let ir = IR {
            category: IRCategory {
                id: 48,
                items: vec![
                    IRItem { id: 10, frn: 1, layout: IRLayout::Fixed { bytes: 2, elements: vec![] } },
                    IRItem { id: 20, frn: 2, layout: IRLayout::Fixed { bytes: 1, elements: vec![] } },
                    IRItem { id: 140, frn: 8, layout: IRLayout::Fixed { bytes: 2, elements: vec![] } },
                ],
            },
        };

        let lowered = lower(&ir);
        let record = &lowered.record;

        assert_eq!(record.entries.len(), 3);
        assert_eq!(record.entries[0].fspec_byte, 0);
        assert_eq!(record.entries[0].fspec_bit, 0);
        assert_eq!(record.entries[1].fspec_byte, 0);
        assert_eq!(record.entries[1].fspec_bit, 1);
        assert_eq!(record.entries[2].fspec_byte, 1);
        assert_eq!(record.entries[2].fspec_bit, 0);
    }

    #[test]
    fn test_lower_extended_item() {
        let ir = IR {
            category: IRCategory {
                id: 48,
                items: vec![IRItem {
                    id: 20,
                    frn: 2,
                    layout: IRLayout::Extended {
                        bytes: 2,
                        part_groups: vec![
                            IRPartGroup {
                                index: 0,
                                elements: vec![
                                    IRElement::Field { name: "a".to_string(), bits: 3, is_string: false },
                                    IRElement::Field { name: "b".to_string(), bits: 4, is_string: false },
                                ],
                            },
                            IRPartGroup {
                                index: 1,
                                elements: vec![
                                    IRElement::Field { name: "c".to_string(), bits: 7, is_string: false },
                                ],
                            },
                        ],
                    },
                }],
            },
        };

        let lowered = lower(&ir);
        let item = &lowered.items[0];

        match &item.kind {
            LoweredItemKind::Extended { parts } => {
                assert_eq!(parts.len(), 2);
                assert!(parts[0].is_required);
                assert!(!parts[1].is_required);
                assert_eq!(parts[0].struct_name, format_ident!("Item020Part0"));
                assert_eq!(parts[1].struct_name, format_ident!("Item020Part1"));
                assert_eq!(parts[0].fields.len(), 2);
                assert_eq!(parts[1].fields.len(), 1);
            }
            _ => panic!("Expected Extended kind"),
        }
    }

    #[test]
    fn test_lower_compound_item() {
        let ir = IR {
            category: IRCategory {
                id: 48,
                items: vec![IRItem {
                    id: 120,
                    frn: 6,
                    layout: IRLayout::Compound {
                        sub_items: vec![
                            IRSubItem {
                                index: 1,
                                layout: IRLayout::Fixed {
                                    bytes: 2,
                                    elements: vec![
                                        IRElement::Field { name: "x".to_string(), bits: 16, is_string: false },
                                    ],
                                },
                            },
                            IRSubItem {
                                index: 2,
                                layout: IRLayout::Fixed {
                                    bytes: 1,
                                    elements: vec![
                                        IRElement::Field { name: "y".to_string(), bits: 8, is_string: false },
                                    ],
                                },
                            },
                        ],
                    },
                }],
            },
        };

        let lowered = lower(&ir);
        let item = &lowered.items[0];

        match &item.kind {
            LoweredItemKind::Compound { sub_items } => {
                assert_eq!(sub_items.len(), 2);
                assert_eq!(sub_items[0].struct_name, format_ident!("Item120Sub1"));
                assert_eq!(sub_items[0].field_name, format_ident!("sub1"));
                assert_eq!(sub_items[0].fspec_byte, 0);
                assert_eq!(sub_items[0].fspec_bit, 0);
            }
            _ => panic!("Expected Compound kind"),
        }
    }

    #[test]
    fn test_lower_string_field() {
        let ir = IR {
            category: IRCategory {
                id: 48,
                items: vec![IRItem {
                    id: 240,
                    frn: 4,
                    layout: IRLayout::Fixed {
                        bytes: 6,
                        elements: vec![
                            IRElement::Field { name: "aircraft_id".to_string(), bits: 48, is_string: true },
                        ],
                    },
                }],
            },
        };

        let lowered = lower(&ir);
        let item = &lowered.items[0];
        assert_eq!(item.name, format_ident!("Item240"));

        match &item.kind {
            LoweredItemKind::Simple { fields, decode_ops, encode_ops, .. } => {
                assert_eq!(fields.len(), 1);
                assert!(matches!(fields[0].type_tokens, FieldType::FixedString(6)));
                assert!(matches!(decode_ops[0], DecodeOp::ReadString { byte_len: 6, .. }));
                assert!(matches!(encode_ops[0], EncodeOp::WriteString { byte_len: 6, .. }));
            }
            _ => panic!("Expected Simple kind"),
        }
    }

    #[test]
    fn test_lower_epb_string_field() {
        let ir = IR {
            category: IRCategory {
                id: 48,
                items: vec![IRItem {
                    id: 30,
                    frn: 3,
                    layout: IRLayout::Fixed {
                        bytes: 7,
                        elements: vec![
                            IRElement::EPB {
                                content: Box::new(IRElement::Field {
                                    name: "callsign".to_string(),
                                    bits: 48,
                                    is_string: true,
                                }),
                            },
                        ],
                    },
                }],
            },
        };

        let lowered = lower(&ir);
        let item = &lowered.items[0];

        match &item.kind {
            LoweredItemKind::Simple { fields, decode_ops, encode_ops, .. } => {
                assert_eq!(fields.len(), 1);
                assert!(matches!(fields[0].type_tokens, FieldType::OptionalFixedString(6)));
                assert!(matches!(decode_ops[0], DecodeOp::ReadEpbString { byte_len: 6, .. }));
                assert!(matches!(encode_ops[0], EncodeOp::WriteEpbString { byte_len: 6, .. }));
            }
            _ => panic!("Expected Simple kind"),
        }
    }
}
