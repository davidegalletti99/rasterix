//! Integration tests for the Builder API.
//!
//! These tests verify that the high-level Builder API correctly
//! orchestrates the parsing, transformation, and code generation pipeline.

use rasterix_codegen::builder::{Builder, RustBuilder};
use std::fs;
use test_utils::{cleanup_temp_files, create_temp_file, load_fixture};

// ============================================================================
// Basic Builder Tests
// ============================================================================

#[test]
fn builder_from_fixture() {
    let temp_path = create_temp_file(&load_fixture("valid", "simple_fixed.xml"), "xml");

    let builder = RustBuilder::new();
    let result = builder.build(temp_path.to_str().unwrap());

    cleanup_temp_files();

    assert!(result.is_ok());
    let code = result.unwrap();
    assert!(code.contains("pub mod cat001"));
}

#[test]
fn builder_generates_record_struct() {
    let temp_path = create_temp_file(&load_fixture("valid", "simple_fixed.xml"), "xml");

    let builder = RustBuilder::new();
    let code = builder.build(temp_path.to_str().unwrap()).unwrap();

    cleanup_temp_files();

    assert!(code.contains("pub mod cat001"));
    assert!(code.contains("pub struct Item010"));
}

#[test]
fn builder_generates_item_structs() {
    let temp_path = create_temp_file(&load_fixture("valid", "multi_item_record.xml"), "xml");

    let builder = RustBuilder::new();
    let code = builder.build(temp_path.to_str().unwrap()).unwrap();

    cleanup_temp_files();

    assert!(code.contains("Item010"));
    assert!(code.contains("Item020"));
}

// ============================================================================
// Build File Tests
// ============================================================================

#[test]
fn builder_build_file() {
    let xml_content = load_fixture("valid", "simple_fixed.xml");

    fs::create_dir_all("target/test_output").unwrap();
    fs::write("target/test_output/test.xml", xml_content).unwrap();

    let builder = RustBuilder::new();
    let result = builder.build_file("target/test_output/test.xml", "target/test_output/generated");

    assert!(result.is_ok());
    let output_path = result.unwrap();
    assert!(output_path.exists());

    let generated = fs::read_to_string(&output_path).unwrap();
    assert!(generated.contains("pub mod cat001"));

    // Cleanup
    fs::remove_dir_all("target/test_output").ok();
}

#[test]
fn builder_creates_output_directory() {
    let xml_content = load_fixture("valid", "simple_fixed.xml");

    // Use a nested directory that doesn't exist
    let output_dir = "target/test_nested/deeply/nested/output";
    fs::create_dir_all("target/test_nested").unwrap();
    fs::write("target/test_nested/test.xml", xml_content).unwrap();

    let builder = RustBuilder::new();
    let result = builder.build_file("target/test_nested/test.xml", output_dir);

    assert!(result.is_ok());
    let output_path = result.unwrap();
    assert!(output_path.exists());

    // Cleanup
    fs::remove_dir_all("target/test_nested").ok();
}

#[test]
fn builder_output_filename_from_input() {
    let xml_content = load_fixture("valid", "simple_fixed.xml");

    fs::create_dir_all("target/test_filename").unwrap();
    fs::write("target/test_filename/cat048.xml", xml_content).unwrap();

    let builder = RustBuilder::new();
    let result = builder.build_file("target/test_filename/cat048.xml", "target/test_filename/out");

    assert!(result.is_ok());
    let output_path = result.unwrap();

    // Output should be named cat048.rs
    assert!(output_path.file_name().unwrap().to_str().unwrap().contains("cat048"));
    assert!(output_path.extension().unwrap() == "rs");

    // Cleanup
    fs::remove_dir_all("target/test_filename").ok();
}

// ============================================================================
// Error Handling Tests
// ============================================================================

#[test]
fn builder_fails_on_missing_file() {
    let builder = RustBuilder::new();
    let result = builder.build("nonexistent_file.xml");

    assert!(result.is_err());
}

#[test]
fn builder_fails_on_invalid_xml() {
    let temp_path = create_temp_file("<invalid xml", "xml");

    let builder = RustBuilder::new();
    let result = builder.build(temp_path.to_str().unwrap());

    cleanup_temp_files();

    assert!(result.is_err());
}

// ============================================================================
// Complex Fixture Tests
// ============================================================================

#[test]
fn builder_handles_extended_item() {
    let temp_path = create_temp_file(&load_fixture("valid", "extended_multi_part.xml"), "xml");

    let builder = RustBuilder::new();
    let result = builder.build(temp_path.to_str().unwrap());

    cleanup_temp_files();

    assert!(result.is_ok());
    let code = result.unwrap();
    assert!(code.contains("Part0"));
    assert!(code.contains("Part1"));
}

#[test]
fn builder_handles_compound_item() {
    let temp_path = create_temp_file(&load_fixture("valid", "compound_simple.xml"), "xml");

    let builder = RustBuilder::new();
    let result = builder.build(temp_path.to_str().unwrap());

    cleanup_temp_files();

    assert!(result.is_ok());
    let code = result.unwrap();
    assert!(code.contains("Sub1"));
    assert!(code.contains("Sub2"));
}

#[test]
fn builder_handles_repetitive_item() {
    let temp_path = create_temp_file(&load_fixture("valid", "repetitive_basic.xml"), "xml");

    let builder = RustBuilder::new();
    let result = builder.build(temp_path.to_str().unwrap());

    cleanup_temp_files();

    assert!(result.is_ok());
    let code = result.unwrap();
    assert!(code.contains("Vec"));
}

#[test]
fn builder_handles_enum() {
    let temp_path = create_temp_file(&load_fixture("valid", "enum_basic.xml"), "xml");

    let builder = RustBuilder::new();
    let result = builder.build(temp_path.to_str().unwrap());

    cleanup_temp_files();

    assert!(result.is_ok());
    let code = result.unwrap();
    assert!(code.contains("enum"));
}

#[test]
fn builder_handles_mixed_all() {
    let temp_path = create_temp_file(&load_fixture("valid", "mixed_all.xml"), "xml");

    let builder = RustBuilder::new();
    let result = builder.build(temp_path.to_str().unwrap());

    cleanup_temp_files();

    assert!(result.is_ok());
}

// ============================================================================
// Default Trait Tests
// ============================================================================

#[test]
fn builder_default_trait() {
    let builder: RustBuilder = Default::default();
    let temp_path = create_temp_file(&load_fixture("valid", "simple_fixed.xml"), "xml");

    let result = builder.build(temp_path.to_str().unwrap());

    cleanup_temp_files();

    assert!(result.is_ok());
}
