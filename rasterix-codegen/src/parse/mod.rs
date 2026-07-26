pub mod xml_model;

use xml_model::Category;

/// Parses the given XML string into a Category struct.
///
/// # Arguments
/// * `xml` - A string slice that holds the XML data.
///
/// # Returns
/// * `Result<Category, quick_xml::DeError>` - The parsed Category or an error if parsing fails.
pub fn parse_category(xml: &str) -> Result<Category, quick_xml::DeError> {
    quick_xml::de::from_str(xml)
}
