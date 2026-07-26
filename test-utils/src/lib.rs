//! Shared test utilities for the rasterix workspace.
//!
//! This crate provides common helpers for loading fixtures, comparing generated code,
//! and other test utilities shared across multiple crates.

use std::fs;
use std::path::PathBuf;

/// Returns the path to the workspace-level testdata directory.
///
/// This resolves the path relative to the workspace root, not the individual crate.
fn testdata_dir() -> PathBuf {
    // CARGO_MANIFEST_DIR points to the crate using this library,
    // so we need to find the workspace root by looking for testdata/
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    // Try common paths to find testdata
    let candidates = [
        manifest_dir.join("../testdata"),      // From test-utils itself
        manifest_dir.join("../../testdata"),   // From sub-crates
        manifest_dir.join("testdata"),         // From workspace root
    ];

    for candidate in &candidates {
        if candidate.exists() {
            return candidate.canonicalize().unwrap_or_else(|_| candidate.clone());
        }
    }

    // Fallback - return the most likely path
    manifest_dir.join("../testdata")
}

/// Returns the path to a fixture file.
///
/// # Arguments
///
/// * `category` - "valid" or "invalid"
/// * `filename` - Name of the XML file (e.g., "simple_fixed.xml")
fn fixture_path(category: &str, filename: &str) -> PathBuf {
    testdata_dir().join(category).join(filename)
}

/// Loads an XML fixture file from the testdata directory.
///
/// # Arguments
///
/// * `category` - "valid" or "invalid"
/// * `filename` - Name of the XML file (e.g., "simple_fixed.xml")
///
/// # Panics
///
/// Panics if the file cannot be read.
pub fn load_fixture(category: &str, filename: &str) -> String {
    let path = fixture_path(category, filename);
    fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read fixture {}: {}", path.display(), e))
}

/// Asserts that generated code contains all expected fragments.
///
/// # Arguments
///
/// * `generated` - The generated code string
/// * `expected_fragments` - Slice of strings that must all appear in the generated code
///
/// # Panics
///
/// Panics with a descriptive message if any fragment is missing.
pub fn assert_code_contains(generated: &str, expected_fragments: &[&str]) {
    for fragment in expected_fragments {
        assert!(
            generated.contains(fragment),
            "Generated code missing fragment: '{}'\n\nGenerated code (first 1000 chars):\n{}",
            fragment,
            &generated.chars().take(1000).collect::<String>()
        );
    }
}

/// Asserts that generated code does NOT contain any of the given fragments.
///
/// # Arguments
///
/// * `generated` - The generated code string
/// * `forbidden_fragments` - Slice of strings that must NOT appear in the generated code
///
/// # Panics
///
/// Panics with a descriptive message if any fragment is found.
pub fn assert_code_not_contains(generated: &str, forbidden_fragments: &[&str]) {
    for fragment in forbidden_fragments {
        assert!(
            !generated.contains(fragment),
            "Generated code contains forbidden fragment: '{fragment}'"
        );
    }
}

/// Creates a temporary test file and returns its path.
///
/// Files are created in the workspace's `target/test_temp/` directory.
///
/// # Arguments
///
/// * `content` - Content to write to the file
/// * `extension` - File extension (e.g., "xml", "rs")
pub fn create_temp_file(content: &str, extension: &str) -> PathBuf {
    use std::io::Write;
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let counter = COUNTER.fetch_add(1, Ordering::SeqCst);

    let filename = format!("rasterix_test_{}_{}.{}", std::process::id(), counter, extension);
    let path = std::env::temp_dir().join(filename);

    let mut file = fs::File::create(&path).expect("Failed to create temp file");
    file.write_all(content.as_bytes()).expect("Failed to write temp file");

    path
}

/// Cleans up temporary test files created by the current process.
pub fn cleanup_temp_files() {
    let prefix = format!("rasterix_test_{}_", std::process::id());
    if let Ok(entries) = fs::read_dir(std::env::temp_dir()) {
        for entry in entries.flatten() {
            if entry.file_name().to_string_lossy().starts_with(&prefix) {
                fs::remove_file(entry.path()).ok();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_assert_code_contains_pass() {
        let code = "pub struct Foo { pub bar: u8 }";
        assert_code_contains(code, &["struct Foo", "pub bar"]);
    }

    #[test]
    #[should_panic(expected = "missing fragment")]
    fn test_assert_code_contains_fail() {
        let code = "pub struct Foo { pub bar: u8 }";
        assert_code_contains(code, &["struct Baz"]);
    }

    #[test]
    fn test_assert_code_not_contains_pass() {
        let code = "pub struct Foo { pub bar: u8 }";
        assert_code_not_contains(code, &["struct Baz", "private"]);
    }

    #[test]
    #[should_panic(expected = "forbidden fragment")]
    fn test_assert_code_not_contains_fail() {
        let code = "pub struct Foo { pub bar: u8 }";
        assert_code_not_contains(code, &["struct Foo"]);
    }
}
