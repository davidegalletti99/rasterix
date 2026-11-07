pub mod spec_model;
pub mod xml_model;

use crate::error::CodegenError;
use spec_model::Asterix;
use xml_model::Category;

/// Parses an XML category definition.
///
/// # Errors
///
/// Returns [`CodegenError::Parse`] if `xml` is malformed or does not match the
/// expected structure.
pub fn parse_category(xml: &str) -> Result<Category, CodegenError> {
    quick_xml::de::from_str(xml).map_err(|source| CodegenError::Parse {
        format: "XML",
        source: Box::new(source),
    })
}

/// Parses an asterix-specs JSON definition (`definition.json`).
///
/// # Errors
///
/// Returns [`CodegenError::Parse`] if `json` is malformed or does not match the
/// asterix-specs schema.
pub fn parse_spec_json(json: &str) -> Result<Asterix, CodegenError> {
    serde_json::from_str(json).map_err(|source| CodegenError::Parse {
        format: "JSON",
        source: Box::new(source),
    })
}
