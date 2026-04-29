use serde::Deserialize;

//
// Top-level structures
//

/// Represents an ASTERIX category definition.
/// At this level, the "category" name defines the structure of a single ASTERIX 
/// data record, not a full data block.
#[derive(Debug, Deserialize)]
pub struct Category {
    #[serde(rename = "@id")]
    pub id: u8,

    #[serde(rename = "item", default)]
    pub items: Vec<Item>,
}

/// Represents a single data item within a category.
#[derive(Debug, Deserialize)]
pub struct Item {
    #[serde(rename = "@id")]
    pub id: u8,

    /// Field Reference Number as defined by the ASTERIX standard (1-indexed).
    /// Per EUROCONTROL-SPEC-0149: FRN 1 is the first item in the record FSPEC,
    /// mapping to the MSB of the first FSPEC byte (0x80).
    #[serde(rename = "@frn")]
    pub frn: u8,

    /// The structural definition of this item
    #[serde(rename = "$value")]
    pub data: ItemStructure,
}

//
// Core structural elements
//

/// Defines the structural type of a data item.
#[derive(Debug, Deserialize)]
pub enum ItemStructure {
    #[serde(rename = "fixed")]
    Fixed(SimpleItem),
    
    #[serde(rename = "explicit")]
    Explicit(SimpleItem),
    
    #[serde(rename = "extended")]
    Extended(ExtendedItem),
    
    #[serde(rename = "repetitive")]
    Repetitive(RepetitiveItem),
    
    #[serde(rename = "compound")]
    Compound(CompoundItem),
}

/// A simple item with a fixed byte size and list of elements.
#[derive(Debug, Deserialize)]
pub struct SimpleItem {
    #[serde(rename = "@bytes")]
    pub bytes: usize,

    #[serde(rename = "$value", default)]
    pub elements: Vec<Element>,
}

/// An extended item with part groups and automatic FX bits.
#[derive(Debug, Deserialize)]
pub struct ExtendedItem {
    #[serde(rename = "@bytes")]
    pub bytes: usize,
    
    #[serde(rename = "part", default)]
    pub part_groups: Vec<PartGroup>,
}

/// A single part group within an extended item.
#[derive(Debug, Deserialize)]
pub struct PartGroup {
    #[serde(rename = "@index")]
    pub index: usize,
    
    #[serde(rename = "$value", default)]
    pub elements: Vec<Element>,
}

/// A repetitive item that repeats a fixed structure N times.
#[derive(Debug, Deserialize)]
pub struct RepetitiveItem {
    #[serde(rename = "@bytes")]
    pub bytes: usize,

    #[serde(rename = "@counter")]
    pub counter: String,

    #[serde(rename = "$value", default)]
    pub elements: Vec<Element>,
}

/// A compound item composed of multiple optional sub-items.
#[derive(Debug, Deserialize)]
pub struct CompoundItem {
    #[serde(rename = "$value", default)]
    pub items: Vec<CompoundableItem>,
}

/// Wrapper for items that can appear within a compound.
#[derive(Debug, Deserialize)]
pub enum CompoundableItem {
    #[serde(rename = "fixed")]
    Fixed(SimpleItem),
    
    #[serde(rename = "explicit")]
    Explicit(SimpleItem),
    
    #[serde(rename = "extended")]
    Extended(ExtendedItem),
    
    #[serde(rename = "repetitive")]
    Repetitive(RepetitiveItem),
}

//
// Leaf / structural nodes
//

/// Individual elements within an item structure.
#[derive(Debug, Deserialize)]
pub enum Element {
    #[serde(rename = "field")]
    Field(Field),
    
    #[serde(rename = "epb")]
    EPB(EPB),
    
    #[serde(rename = "enum")]
    Enum(Enum),
    
    #[serde(rename = "spare")]
    Spare(Spare),
}

/// A basic data field.
#[derive(Debug, Deserialize)]
pub struct Field {
    #[serde(rename = "@name")]
    pub name: String,

    #[serde(rename = "@bits")]
    pub bits: usize,

    // defines the type of the field, e.g., "string" or "numeric"
    #[serde(rename = "@type", default = "default_type")]
    pub field_type: String,
}
/// Default value for the type field.
fn default_type() -> String {
    "numeric".into()
}


/// Extended Primary Bit (EPB) - a field/enum with an automatic presence bit.
#[derive(Debug, Deserialize)]
pub struct EPB {
    #[serde(rename = "$value")]
    pub content: EPBContent,
}

/// The content of an EPB can be either a field or an enum.
#[derive(Debug, Deserialize)]
pub enum EPBContent {
    #[serde(rename = "field")]
    Field(Field),
    
    #[serde(rename = "enum")]
    Enum(Enum),
}

/// Spare/unused bits in the data structure.
#[derive(Debug, Deserialize)]
pub struct Spare {
    #[serde(rename = "@bits")]
    pub bits: usize,
}

/// An enumeration field with named values.
#[derive(Debug, Deserialize)]
pub struct Enum {
    #[serde(rename = "@name")]
    pub name: String,

    #[serde(rename = "@bits")]
    pub bits: usize,

    #[serde(rename = "value", default)]
    pub values: Vec<Value>,
}

/// A single value within an enumeration.
#[derive(Debug, Deserialize)]
pub struct Value {
    #[serde(rename = "@name")]
    pub name: String,

    #[serde(rename = "@value")]
    pub value: String,
}