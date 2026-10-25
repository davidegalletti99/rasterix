pub mod xml_model;

use crate::error::CodegenError;
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
