//! Integration tests for code generation.
//!
//! These tests verify that the code generator produces correct Rust code
//! from the intermediate representation (IR).

use rasterix_codegen::generate::generate;
use rasterix_codegen::parse::parser::parse_category;
use rasterix_codegen::transform::transformer::to_ir;
use test_utils::{
    assert_code_contains, assert_code_not_contains, load_fixture,
};

/// Helper function to generate code from a fixture file.
fn generate_from_fixture(category: &str, filename: &str) -> String {
    let xml = load_fixture(category, filename);
    let parsed = parse_category(&xml).expect("Failed to parse XML fixture");
    let ir = to_ir(parsed);
    let tokens = generate(&ir);
    tokens.to_string()
}

// ============================================================================
// Basic Code Generation Tests
// ============================================================================

#[test]
fn generate_simple_fixed_code() {
    let code = generate_from_fixture("valid", "simple_fixed.xml");

    assert_code_contains(&code, &[
        "pub struct Record",
        "pub struct Item010",
        "pub sac",
        "pub sic",
    ]);
}

#[test]
fn generate_includes_imports() {
    let code = generate_from_fixture("valid", "simple_fixed.xml");

    assert_code_contains(&code, &[
        "use rasterix",
        "BitReader",
        "BitWriter",
        "DecodeError",
        "Decode",
        "Encode",
    ]);
}

#[test]
fn generate_includes_derive_macros() {
    let code = generate_from_fixture("valid", "simple_fixed.xml");

    assert_code_contains(&code, &[
        "derive",
        "Debug",
        "Clone",
        "PartialEq",
    ]);
}

// ============================================================================
// Extended Item Code Generation
// ============================================================================

#[test]
fn generate_extended_code() {
    let code = generate_from_fixture("valid", "extended_multi_part.xml");

    assert_code_contains(&code, &[
        "pub struct Item020Part0",
        "pub struct Item020Part1",
        "pub part0",
        "pub part1 : Option",
    ]);
}

#[test]
fn generate_extended_single_part() {
    let code = generate_from_fixture("valid", "extended_single_part.xml");

    // Should have at least one part struct
    assert_code_contains(&code, &["Part0"]);
}

// ============================================================================
// Enum Code Generation
// ============================================================================

#[test]
fn generate_enum_code() {
    let code = generate_from_fixture("valid", "enum_basic.xml");

    assert_code_contains(&code, &[
        "pub enum TargetType",
        "Psr = 1u8",
        "Ssr = 2u8",
        "Unknown (u8)",
        "impl TryFrom < u8 > for TargetType",
    ]);
}

// ============================================================================
// EPB (Extend Presence Bit) Code Generation
// ============================================================================

#[test]
fn generate_epb_code() {
    let code = generate_from_fixture("valid", "epb_field.xml");

    // EPB should generate Option<T>
    assert_code_contains(&code, &[
        "pub optional_value : Option",
    ]);

    // Should NOT have a separate "name" field for the EPB itself
    assert_code_not_contains(&code, &[
        "pub name",
    ]);
}

#[test]
fn generate_epb_enum_code() {
    let code = generate_from_fixture("valid", "epb_enum.xml");

    // EPB with enum should generate Option<EnumType>
    assert_code_contains(&code, &["Option"]);
}

// ============================================================================
// Compound Item Code Generation
// ============================================================================

#[test]
fn generate_compound_code() {
    let code = generate_from_fixture("valid", "compound_simple.xml");

    assert_code_contains(&code, &[
        "Item100Sub1",
        "Item100Sub2",
        "pub sub1 : Option",
        "pub sub2 : Option",
    ]);
}

#[test]
fn generate_compound_complex_code() {
    let code = generate_from_fixture("valid", "compound_complex.xml");

    // Just verify it generates without error
    assert_code_contains(&code, &["pub struct"]);
}

// ============================================================================
// Repetitive Item Code Generation
// ============================================================================

#[test]
fn generate_repetitive_code() {
    let code = generate_from_fixture("valid", "repetitive_basic.xml");

    assert_code_contains(&code, &[
        "Item070Element",
        "pub items : Vec < Item070Element >",
    ]);
}

#[test]
fn generate_repetitive_with_epb_code() {
    let code = generate_from_fixture("valid", "repetitive_with_epb.xml");

    // Should have Vec and possibly Option types
    assert_code_contains(&code, &["Vec"]);
}

// ============================================================================
// Explicit Item Code Generation
// ============================================================================

#[test]
fn generate_explicit_item_code() {
    let code = generate_from_fixture("valid", "explicit_item.xml");

    assert_code_contains(&code, &[
        "pub struct Item060",
        "pub altitude",
        "pub speed",
    ]);
}

// ============================================================================
// Spare Bits Handling
// ============================================================================

#[test]
fn spare_bits_not_in_struct() {
    let code = generate_from_fixture("valid", "spare_bits.xml");

    // Spare bits should be read/written but not appear as struct fields
    assert_code_contains(&code, &["pub data"]);
    assert_code_not_contains(&code, &["spare", "pub spare"]);
}

// ============================================================================
// Record Generation
// ============================================================================

#[test]
fn generate_record_struct() {
    let code = generate_from_fixture("valid", "multi_item_record.xml");

    assert_code_contains(&code, &[
        "pub struct Record",
        "pub item010 : Option < Item010 >",
        "pub item020 : Option < Item020 >",
    ]);
}

#[test]
fn generate_record_fspec_handling() {
    let code = generate_from_fixture("valid", "multi_item_record.xml");

    assert_code_contains(&code, &[
        "let fspec = Fspec :: read",
        "fspec . is_set",
        "fspec . set",
    ]);
}

// ============================================================================
// Decode Implementation Tests
// ============================================================================

#[test]
fn generate_decode_impl() {
    let code = generate_from_fixture("valid", "simple_fixed.xml");

    assert_code_contains(&code, &[
        "impl Decode for Item010",
        "fn decode",
        "read_bits",
    ]);
}

// ============================================================================
// Encode Implementation Tests
// ============================================================================

#[test]
fn generate_encode_impl() {
    let code = generate_from_fixture("valid", "simple_fixed.xml");

    assert_code_contains(&code, &[
        "impl Encode for Item010",
        "fn encode",
        "write_bits",
    ]);
}


// ============================================================================
// String Field Code Generation
// ============================================================================

#[test]
fn generate_string_field_struct_type() {
    let code = generate_from_fixture("valid", "multi_item_record.xml");

    // String field should generate a String type, not a numeric type
    assert_code_contains(&code, &[
        "pub struct Item240",
        "pub aircraft_id : String",
    ]);
}

#[test]
fn generate_string_field_decode() {
    let code = generate_from_fixture("valid", "multi_item_record.xml");

    // String decode should use read_string, not read_bits
    assert_code_contains(&code, &[
        "read_string (6usize)",
    ]);
}

#[test]
fn generate_string_field_encode() {
    let code = generate_from_fixture("valid", "multi_item_record.xml");

    // String encode should use write_string, not write_bits
    assert_code_contains(&code, &[
        "write_string (& self . aircraft_id , 6usize)",
    ]);
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn generate_handles_mixed_all() {
    let code = generate_from_fixture("valid", "mixed_all.xml");

    // Just verify it generates without error
    assert!(!code.is_empty());
    assert_code_contains(&code, &["pub struct"]);
}
