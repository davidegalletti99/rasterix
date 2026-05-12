//! Integration tests for XML parsing.
//!
//! These tests verify that the XML parser correctly transforms ASTERIX XML
//! definitions into the xml_model data structures.

use rasterix_codegen::parse::parser::parse_category;
use rasterix_codegen::parse::xml_model::*;
use test_utils::load_fixture;

// ============================================================================
// Basic Parsing Tests
// ============================================================================

#[test]
fn parse_simple_fixed_item() {
    let xml = load_fixture("valid", "simple_fixed.xml");
    let category = parse_category(&xml).expect("Failed to parse XML");

    assert_eq!(category.id, 1);
    assert_eq!(category.items.len(), 1);
    assert_eq!(category.items[0].id, 10);
    assert_eq!(category.items[0].frn, 1);
}

#[test]
fn parse_fixed_item_structure() {
    let xml = load_fixture("valid", "simple_fixed.xml");
    let category = parse_category(&xml).expect("Failed to parse XML");

    match &category.items[0].data {
        ItemStructure::Fixed(fixed) => {
            assert_eq!(fixed.bytes, 2);
            assert_eq!(fixed.elements.len(), 2);

            // Check first field
            match &fixed.elements[0] {
                Element::Field(field) => {
                    assert_eq!(field.name, "sac");
                    assert_eq!(field.bits, 8);
                }
                _ => panic!("Expected Field element"),
            }

            // Check second field
            match &fixed.elements[1] {
                Element::Field(field) => {
                    assert_eq!(field.name, "sic");
                    assert_eq!(field.bits, 8);
                }
                _ => panic!("Expected Field element"),
            }
        }
        _ => panic!("Expected Fixed item"),
    }
}

// ============================================================================
// Extended Item Tests
// ============================================================================

#[test]
fn parse_extended_item() {
    let xml = load_fixture("valid", "extended_multi_part.xml");
    let category = parse_category(&xml).expect("Failed to parse XML");

    match &category.items[0].data {
        ItemStructure::Extended(ext) => {
            assert_eq!(ext.bytes, 3);
            assert_eq!(ext.part_groups.len(), 3);
        }
        _ => panic!("Expected Extended item"),
    }
}

#[test]
fn parse_extended_single_part() {
    let xml = load_fixture("valid", "extended_single_part.xml");
    let category = parse_category(&xml).expect("Failed to parse XML");

    match &category.items[0].data {
        ItemStructure::Extended(ext) => {
            assert!(!ext.part_groups.is_empty());
        }
        _ => panic!("Expected Extended item"),
    }
}

// ============================================================================
// EPB (Extend Presence Bit) Tests
// ============================================================================

#[test]
fn parse_epb_field() {
    let xml = load_fixture("valid", "epb_field.xml");
    let category = parse_category(&xml).expect("Failed to parse XML");

    match &category.items[0].data {
        ItemStructure::Fixed(fixed) => {
            match &fixed.elements[0] {
                Element::EPB(epb) => {
                    match &epb.content {
                        EPBContent::Field(field) => {
                            assert_eq!(field.name, "optional_value");
                            assert_eq!(field.bits, 15);
                        }
                        _ => panic!("Expected Field in EPB"),
                    }
                }
                _ => panic!("Expected EPB element"),
            }
        }
        _ => panic!("Expected Fixed item"),
    }
}

#[test]
fn parse_epb_enum() {
    let xml = load_fixture("valid", "epb_enum.xml");
    let category = parse_category(&xml).expect("Failed to parse XML");

    match &category.items[0].data {
        ItemStructure::Fixed(fixed) => {
            // Find the EPB element
            let has_epb_with_enum = fixed.elements.iter().any(|e| {
                matches!(e, Element::EPB(epb) if matches!(&epb.content, EPBContent::Enum(_)))
            });
            assert!(has_epb_with_enum, "Expected EPB with Enum content");
        }
        _ => panic!("Expected Fixed item"),
    }
}

// ============================================================================
// Enum Tests
// ============================================================================

#[test]
fn parse_enum() {
    let xml = load_fixture("valid", "enum_basic.xml");
    let category = parse_category(&xml).expect("Failed to parse XML");

    match &category.items[0].data {
        ItemStructure::Fixed(fixed) => {
            match &fixed.elements[0] {
                Element::Enum(enum_def) => {
                    assert_eq!(enum_def.name, "target_type");
                    assert_eq!(enum_def.bits, 3);
                    assert_eq!(enum_def.values.len(), 2);
                }
                _ => panic!("Expected Enum element"),
            }
        }
        _ => panic!("Expected Fixed item"),
    }
}

// ============================================================================
// Compound Item Tests
// ============================================================================

#[test]
fn parse_compound_simple() {
    let xml = load_fixture("valid", "compound_simple.xml");
    let category = parse_category(&xml).expect("Failed to parse XML");

    match &category.items[0].data {
        ItemStructure::Compound(compound) => {
            assert!(!compound.items.is_empty());
        }
        _ => panic!("Expected Compound item"),
    }
}

#[test]
fn parse_compound_complex() {
    let xml = load_fixture("valid", "compound_complex.xml");
    let category = parse_category(&xml).expect("Failed to parse XML");

    match &category.items[0].data {
        ItemStructure::Compound(_) => {
            // Just verify it parses without error
        }
        _ => panic!("Expected Compound item"),
    }
}

// ============================================================================
// Repetitive Item Tests
// ============================================================================

#[test]
fn parse_repetitive_basic() {
    let xml = load_fixture("valid", "repetitive_basic.xml");
    let category = parse_category(&xml).expect("Failed to parse XML");

    match &category.items[0].data {
        ItemStructure::Repetitive(rep) => {
            assert!(!rep.elements.is_empty());
        }
        _ => panic!("Expected Repetitive item"),
    }
}

#[test]
fn parse_repetitive_with_epb() {
    let xml = load_fixture("valid", "repetitive_with_epb.xml");
    let category = parse_category(&xml).expect("Failed to parse XML");

    match &category.items[0].data {
        ItemStructure::Repetitive(_) => {
            // Just verify it parses without error
        }
        _ => panic!("Expected Repetitive item"),
    }
}

// ============================================================================
// Explicit Item Tests
// ============================================================================

#[test]
fn parse_explicit_item() {
    let xml = load_fixture("valid", "explicit_item.xml");
    let category = parse_category(&xml).expect("Failed to parse XML");

    match &category.items[0].data {
        ItemStructure::Explicit(_) => {
            // Just verify it parses without error
        }
        _ => panic!("Expected Explicit item"),
    }
}

// ============================================================================
// Spare Bits Tests
// ============================================================================

#[test]
fn parse_spare_bits() {
    let xml = load_fixture("valid", "spare_bits.xml");
    let category = parse_category(&xml).expect("Failed to parse XML");

    match &category.items[0].data {
        ItemStructure::Fixed(fixed) => {
            let has_spare = fixed.elements.iter().any(|e| matches!(e, Element::Spare(_)));
            assert!(has_spare, "Expected Spare element in item");
        }
        _ => panic!("Expected Fixed item with spare bits"),
    }
}

// ============================================================================
// Multi-Item Record Tests
// ============================================================================

#[test]
fn parse_multi_item_record() {
    let xml = load_fixture("valid", "multi_item_record.xml");
    let category = parse_category(&xml).expect("Failed to parse XML");

    assert_eq!(category.id, 48);
    assert!(category.items.len() >= 2);
}

// ============================================================================
// Edge Case Tests
// ============================================================================

#[test]
fn parse_same_name_different_items() {
    let xml = load_fixture("valid", "same_name_different_items.xml");
    let result = parse_category(&xml);
    assert!(result.is_ok());
}

#[test]
fn parse_same_name_different_parts() {
    let xml = load_fixture("valid", "same_name_different_parts.xml");
    let result = parse_category(&xml);
    assert!(result.is_ok());
}

#[test]
fn parse_same_name_different_subitems() {
    let xml = load_fixture("valid", "same_name_different_subitems.xml");
    let result = parse_category(&xml);
    assert!(result.is_ok());
}

#[test]
fn parse_mixed_all() {
    let xml = load_fixture("valid", "mixed_all.xml");
    let result = parse_category(&xml);
    assert!(result.is_ok());
}

// ============================================================================
// Invalid XML Tests
// ============================================================================

#[test]
fn parse_invalid_xml_fails() {
    let xml = r#"<?xml version="1.0"?><not-valid"#;
    let result = parse_category(xml);
    assert!(result.is_err());
}

#[test]
fn parse_empty_xml_fails() {
    let xml = "";
    let result = parse_category(xml);
    assert!(result.is_err());
}
