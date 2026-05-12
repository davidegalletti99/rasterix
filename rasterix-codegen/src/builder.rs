use std::{fs, path::PathBuf};
use crate::{
    generate::generate,
    parse::parser::parse_category,
    transform::transformer::to_ir,
    CodegenError,
};

/// Trait for building ASTERIX code from XML definitions.
pub trait Builder {
    /// Builds Rust code from an XML file.
    ///
    /// # Arguments
    ///
    /// * `file_path` - Path to the XML file
    ///
    /// # Returns
    ///
    /// The generated Rust code as a string.
    ///
    /// # Errors
    ///
    /// Returns [`CodegenError::Io`] if the file cannot be read.
    /// Returns [`CodegenError::Parse`] if the XML is malformed.
    /// Returns other [`CodegenError`] variants if the definition fails validation or lowering.
    fn build(&self, file_path: &str) -> Result<String, CodegenError>;
}

/// Rust code generator builder.
pub struct RustBuilder;

impl Builder for RustBuilder {
    fn build(&self, file_path: &str) -> Result<String, CodegenError> {
        // Read XML file
        let xml = fs::read_to_string(file_path)
            .map_err(|source| CodegenError::Io { path: file_path.to_string(), source })?;

        // Parse XML into model
        let category = parse_category(&xml)?;

        // Transform to IR (validates at this stage)
        let ir = to_ir(category)?;

        // Generate Rust code
        let tokens = generate(&ir)?;

        Ok(tokens.to_string())
    }
}

impl RustBuilder {
    /// Creates a new RustBuilder instance.
    pub fn new() -> Self {
        Self
    }
    
    /// Builds code from a single file and writes to output directory.
    ///
    /// # Arguments
    ///
    /// * `input_path` - Path to the XML file
    /// * `output_dir` - Directory to write generated code
    ///
    /// # Returns
    ///
    /// Path to the generated file.
    ///
    /// # Errors
    ///
    /// Returns [`CodegenError`] from [`Builder::build`] if generation fails.
    /// Returns [`CodegenError::Io`] if the output directory cannot be created or the file cannot be written.
    pub fn build_file(
        &self,
        input_path: &str,
        output_dir: &str,
    ) -> Result<PathBuf, CodegenError> {
        let code = self.build(input_path)?;

        // Extract category number from generated code or filename
        let output_filename = Self::extract_output_filename(input_path);
        let output_path = PathBuf::from(output_dir).join(output_filename);

        // Create output directory if needed
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|source| CodegenError::Io { path: parent.display().to_string(), source })?;
        }

        // Write generated code
        fs::write(&output_path, code)
            .map_err(|source| CodegenError::Io { path: output_path.display().to_string(), source })?;

        Ok(output_path)
    }
    
    /// Builds code from all XML files in a directory.
    ///
    /// Per-file generation failures are logged to stderr and skipped; only directory-level
    /// I/O errors (unreadable directory, non-UTF-8 path) cause the method to return `Err`.
    ///
    /// # Arguments
    ///
    /// * `input_dir` - Directory containing XML files
    /// * `output_dir` - Directory to write generated code
    ///
    /// # Returns
    ///
    /// Vector of paths to successfully generated files.
    ///
    /// # Errors
    ///
    /// Returns [`CodegenError::Io`] if `input_dir` cannot be read.
    /// Returns [`CodegenError::InvalidPath`] if any entry path contains non-UTF-8 bytes.
    pub fn build_directory(
        &self,
        input_dir: &str,
        output_dir: &str,
    ) -> Result<Vec<PathBuf>, CodegenError> {
        let mut generated_files = Vec::new();

        // Read directory
        let entries = fs::read_dir(input_dir)
            .map_err(|source| CodegenError::Io { path: input_dir.to_string(), source })?;

        for entry in entries {
            let entry = entry
                .map_err(|source| CodegenError::Io { path: input_dir.to_string(), source })?;
            let path = entry.path();

            // Process only .xml files
            if path.extension().and_then(|s| s.to_str()) == Some("xml") {
                let input_path = path.to_str()
                    .ok_or(CodegenError::InvalidPath)?;

                match self.build_file(input_path, output_dir) {
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
}

impl Default for RustBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_extract_output_filename() {
        assert_eq!(
            RustBuilder::extract_output_filename("cat048.xml"),
            "cat048.rs"
        );
        assert_eq!(
            RustBuilder::extract_output_filename("/path/to/cat001.xml"),
            "cat001.rs"
        );
        assert_eq!(
            RustBuilder::extract_output_filename("test.xml"),
            "test.rs"
        );
    }
}