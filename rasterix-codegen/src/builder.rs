use std::{fs, path::PathBuf};
use crate::{
    generate::generate,
    parse::parse_category,
    transform::transformer::to_ir,
    CodegenError,
};

/// Builds Rust code from an in-memory XML string.
///
/// # Errors
///
/// Returns [`CodegenError::Parse`] if the XML is malformed.
/// Returns other [`CodegenError`] variants if the definition fails validation or lowering.
pub fn build_str(xml: &str) -> Result<String, CodegenError> {
    let category = parse_category(xml)?;
    let ir = to_ir(category)?;
    let tokens = generate(&ir)?;

    Ok(tokens.to_string())
}

/// Builds Rust code from an XML file.
///
/// # Errors
///
/// Returns [`CodegenError::Io`] if the file cannot be read.
/// Returns [`CodegenError::Parse`] if the XML is malformed.
/// Returns other [`CodegenError`] variants if the definition fails validation or lowering.
pub fn build(file_path: &str) -> Result<String, CodegenError> {
    let xml = fs::read_to_string(file_path)
        .map_err(|source| CodegenError::Io { path: file_path.to_string(), source })?;
    build_str(&xml)
}

/// Builds code from a single file and writes it to the output directory.
///
/// Returns the path to the generated file.
///
/// # Errors
///
/// Returns [`CodegenError`] from [`build`] if generation fails.
/// Returns [`CodegenError::Io`] if the output directory cannot be created or the file cannot be written.
pub fn build_file(input_path: &str, output_dir: &str) -> Result<PathBuf, CodegenError> {
    let code = build(input_path)?;

    let output_filename = extract_output_filename(input_path);
    let output_path = PathBuf::from(output_dir).join(output_filename);

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|source| CodegenError::Io { path: parent.display().to_string(), source })?;
    }

    fs::write(&output_path, code)
        .map_err(|source| CodegenError::Io { path: output_path.display().to_string(), source })?;

    Ok(output_path)
}

/// Builds code from all XML files in a directory.
///
/// Per-file generation failures are logged to stderr and skipped; only directory-level
/// I/O errors (unreadable directory, non-UTF-8 path) cause this to return `Err`.
///
/// # Errors
///
/// Returns [`CodegenError::Io`] if `input_dir` cannot be read.
/// Returns [`CodegenError::InvalidPath`] if any entry path contains non-UTF-8 bytes.
pub fn build_directory(input_dir: &str, output_dir: &str) -> Result<Vec<PathBuf>, CodegenError> {
    let mut generated_files = Vec::new();

    let entries = fs::read_dir(input_dir)
        .map_err(|source| CodegenError::Io { path: input_dir.to_string(), source })?;

    for entry in entries {
        let entry = entry
            .map_err(|source| CodegenError::Io { path: input_dir.to_string(), source })?;
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("xml") {
            let input_path = path.to_str()
                .ok_or(CodegenError::InvalidPath)?;

            match build_file(input_path, output_dir) {
                Ok(output_path) => {
                    println!("Generated: {output_path:?}");
                    generated_files.push(output_path);
                }
                Err(e) => {
                    eprintln!("Warning: Failed to process {input_path}: {e}");
                }
            }
        }
    }

    Ok(generated_files)
}

/// Extracts the output filename from the input path.
///
/// For example: "cat048.xml" -> "cat048.rs"
fn extract_output_filename(input_path: &str) -> String {
    PathBuf::from(input_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|s| format!("{s}.rs"))
        .unwrap_or_else(|| "generated.rs".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_output_filename() {
        assert_eq!(extract_output_filename("cat048.xml"), "cat048.rs");
        assert_eq!(extract_output_filename("/path/to/cat001.xml"), "cat001.rs");
        assert_eq!(extract_output_filename("test.xml"), "test.rs");
    }
}
