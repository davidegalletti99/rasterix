//! Integration tests for XML to IR transformation.
//!
//! These tests verify that the transformer correctly converts xml_model
//! structures into the intermediate representation (IR) and validates
//! the resulting structures.

use rasterix_codegen::parse::parser::parse_category;
use rasterix_codegen::transform::ir::*;
use rasterix_codegen::transform::transformer::to_ir;
use test_utils::load_fixture;

/// Helper function to build IR from a fixture file.
fn build_ir_from_fixture(category: &str, filename: &str) -> IR {
    let xml = load_fixture(category, filename);
    let parsed = parse_category(&xml).expect("Failed to parse XML fixture");
    to_ir(parsed)
}

// ============================================================================
// Basic Transformation Tests
// ============================================================================

#[test]
fn transform_simple_fixed_to_ir() {
    let ir = build_ir_from_fixture("valid", "simple_fixed.xml");

    assert_eq!(ir.category.id, 1);
    assert_eq!(ir.category.items.len(), 1);
    assert_eq!(ir.category.items[0].id, 10);
    assert_eq!(ir.category.items[0].frn, 1);
}

#[test]
fn transform_preserves_item_order() {
    let ir = build_ir_from_fixture("valid", "multi_item_record.xml");

    assert_eq!(ir.category.id, 48);
    assert!(ir.category.items.len() >= 2);

    // Items should be in order by their position in XML
    let ids: Vec<u8> = ir.category.items.iter().map(|i| i.id).collect();
    assert_eq!(ids[0], 10);
    assert_eq!(ids[1], 20);
}

// ============================================================================
// Layout Transformation Tests
// ============================================================================

#[test]
fn transform_fixed_layout() {
    let ir = build_ir_from_fixture("valid", "simple_fixed.xml");

    match &ir.category.items[0].layout {
        IRLayout::Fixed { bytes, elements } => {
            assert_eq!(*bytes, 2);
            assert_eq!(elements.len(), 2);
        }
        _ => panic!("Expected Fixed layout"),
    }
}

#[test]
fn transform_extended_layout() {
    let ir = build_ir_from_fixture("valid", "extended_multi_part.xml");

    match &ir.category.items[0].layout {
        IRLayout::Extended { bytes, part_groups } => {
            assert_eq!(*bytes, 3);
            assert_eq!(part_groups.len(), 3);

            // Check part group indices
            assert_eq!(part_groups[0].index, 0);
            assert_eq!(part_groups[1].index, 1);
            assert_eq!(part_groups[2].index, 2);
        }
        _ => panic!("Expected Extended layout"),
    }
}

#[test]
fn transform_compound_layout() {
    let ir = build_ir_from_fixture("valid", "compound_simple.xml");

    match &ir.category.items[0].layout {
        IRLayout::Compound { sub_items } => {
            assert!(!sub_items.is_empty());

            // Check sub-item indices (1-indexed per ASTERIX standard)
            for (i, sub_item) in sub_items.iter().enumerate() {
                assert_eq!(sub_item.index, i + 1);
            }
        }
        _ => panic!("Expected Compound layout"),
    }
}

#[test]
fn transform_repetitive_layout() {
    let ir = build_ir_from_fixture("valid", "repetitive_basic.xml");

    match &ir.category.items[0].layout {
        IRLayout::Repetitive { bytes, counter_bytes, elements } => {
            assert!(*bytes > 0);
            assert!(*counter_bytes > 0);
            assert!(!elements.is_empty());
        }
        _ => panic!("Expected Repetitive layout"),
    }
}

#[test]
fn transform_explicit_layout() {
    let ir = build_ir_from_fixture("valid", "explicit_item.xml");

    match &ir.category.items[0].layout {
        IRLayout::Explicit { bytes, elements } => {
            assert!(*bytes > 0);
            assert!(!elements.is_empty());
        }
        _ => panic!("Expected Explicit layout"),
    }
}

// ============================================================================
// Element Transformation Tests
// ============================================================================

#[test]
fn transform_field_element() {
    let ir = build_ir_from_fixture("valid", "simple_fixed.xml");

    match &ir.category.items[0].layout {
        IRLayout::Fixed { elements, .. } => {
            match &elements[0] {
                IRElement::Field { name, bits, is_string } => {
                    assert_eq!(name, "sac");
                    assert_eq!(*bits, 8);
                    assert_eq!(*is_string, false);
                }
                _ => panic!("Expected Field element"),
            }
        }
        _ => panic!("Expected Fixed layout"),
    }
}

#[test]
fn transform_enum_element() {
    let ir = build_ir_from_fixture("valid", "enum_basic.xml");

    match &ir.category.items[0].layout {
        IRLayout::Fixed { elements, .. } => {
            match &elements[0] {
                IRElement::Enum { name, bits, values } => {
                    assert_eq!(name, "target_type");
                    assert_eq!(*bits, 3);
                    assert!(!values.is_empty());
                }
                _ => panic!("Expected Enum element"),
            }
        }
        _ => panic!("Expected Fixed layout"),
    }
}

#[test]
fn transform_epb_element() {
    let ir = build_ir_from_fixture("valid", "epb_field.xml");

    match &ir.category.items[0].layout {
        IRLayout::Fixed { elements, .. } => {
            let has_epb = elements.iter().any(|e| matches!(e, IRElement::EPB { .. }));
            assert!(has_epb, "Expected EPB element in IR");
        }
        _ => panic!("Expected Fixed layout"),
    }
}

#[test]
fn transform_spare_element() {
    let ir = build_ir_from_fixture("valid", "spare_bits.xml");

    match &ir.category.items[0].layout {
        IRLayout::Fixed { elements, .. } => {
            let has_spare = elements.iter().any(|e| matches!(e, IRElement::Spare { .. }));
            assert!(has_spare, "Expected Spare element in IR");
        }
        _ => panic!("Expected Fixed layout"),
    }
}

// ============================================================================
// Validation Tests
// ============================================================================

#[test]
#[should_panic(expected = "Bit count mismatch")]
fn validation_rejects_bit_mismatch() {
    let _ = build_ir_from_fixture("invalid", "bit_mismatch.xml");
}

#[test]
#[should_panic(expected = "Part group")]
fn validation_rejects_extended_bit_mismatch() {
    let _ = build_ir_from_fixture("invalid", "extended_bit_mismatch.xml");
}

// ============================================================================
// Complex Structure Tests
// ============================================================================

#[test]
fn transform_compound_with_nested_layouts() {
    let ir = build_ir_from_fixture("valid", "compound_complex.xml");

    match &ir.category.items[0].layout {
        IRLayout::Compound { sub_items } => {
            // Verify each sub-item has a valid layout
            for sub_item in sub_items {
                match &sub_item.layout {
                    IRLayout::Fixed { .. } |
                    IRLayout::Extended { .. } |
                    IRLayout::Repetitive { .. } |
                    IRLayout::Explicit { .. } => {
                        // Valid nested layout types
                    }
                    IRLayout::Compound { .. } => {
                        panic!("Compound should not nest Compound directly");
                    }
                }
            }
        }
        _ => panic!("Expected Compound layout"),
    }
}

#[test]
fn transform_mixed_all() {
    // This fixture should contain multiple item types
    let ir = build_ir_from_fixture("valid", "mixed_all.xml");

    // Just verify it transforms without error
    assert!(!ir.category.items.is_empty());
}

// ============================================================================
// Edge Case Tests
// ============================================================================

#[test]
fn transform_handles_same_name_different_items() {
    let ir = build_ir_from_fixture("valid", "same_name_different_items.xml");
    assert!(!ir.category.items.is_empty());
}

#[test]
fn transform_handles_same_name_different_parts() {
    let ir = build_ir_from_fixture("valid", "same_name_different_parts.xml");
    assert!(!ir.category.items.is_empty());
}

#[test]
fn transform_handles_same_name_different_subitems() {
    let ir = build_ir_from_fixture("valid", "same_name_different_subitems.xml");
    assert!(!ir.category.items.is_empty());
}
