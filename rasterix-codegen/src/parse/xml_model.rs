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

    /// The User Application Profile, placing the items in the record
    pub uap: Uap,
}

/// The User Application Profile: the record's FSPEC slots in FRN order.
#[derive(Debug, Deserialize)]
pub struct Uap {
    #[serde(rename = "$value", default)]
    pub slots: Vec<UapEntry>,
}

/// One FSPEC slot of the UAP.
#[derive(Debug, Deserialize)]
pub enum UapEntry {
    /// `<entry item="010"/>`: the slot holds the item with this id.
    #[serde(rename = "entry")]
    Entry {
        #[serde(rename = "@item")]
        item: String,
    },

    /// `<spare/>`: an unassigned FRN (asterix-specs' `-`).
    #[serde(rename = "spare")]
    Spare(CompoundSpare),
}

/// Represents a single data item within a category.
#[derive(Debug, Deserialize)]
pub struct Item {
    /// Data Item identifier, e.g. "010" for I048/010 or "SP". It names the
    /// generated struct and the item's UAP entry, and never reaches the wire.
    #[serde(rename = "@id")]
    pub id: String,

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
    Explicit(ExplicitItem),
    
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
    /// Sub-item name, required when this structure is a compound child
    #[serde(rename = "@id", default)]
    pub id: Option<String>,

    #[serde(rename = "@bytes")]
    pub bytes: usize,

    #[serde(rename = "$value", default)]
    pub elements: Vec<Element>,
}

/// An explicit item: a length byte followed by the payload. Without `bytes`
/// the payload is opaque and its length is only known at runtime.
#[derive(Debug, Deserialize)]
pub struct ExplicitItem {
    /// Sub-item name, required when this structure is a compound child
    #[serde(rename = "@id", default)]
    pub id: Option<String>,

    #[serde(rename = "@bytes")]
    pub bytes: Option<usize>,

    #[serde(rename = "$value", default)]
    pub elements: Vec<Element>,
}

/// An extended item with part groups and automatic FX bits.
#[derive(Debug, Deserialize)]
pub struct ExtendedItem {
    /// Sub-item name, required when this structure is a compound child
    #[serde(rename = "@id", default)]
    pub id: Option<String>,

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
    /// Sub-item name, required when this structure is a compound child
    #[serde(rename = "@id", default)]
    pub id: Option<String>,

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
    Explicit(ExplicitItem),
    
    #[serde(rename = "extended")]
    Extended(ExtendedItem),
    
    #[serde(rename = "repetitive")]
    Repetitive(RepetitiveItem),

    /// An unassigned FSPEC bit slot: `<spare/>` (no attributes) skips one
    /// compound FSPEC position without defining a sub-item.
    #[serde(rename = "spare")]
    Spare(CompoundSpare),
}

/// Empty marker for an unassigned compound FSPEC slot.
#[derive(Debug, Deserialize)]
pub struct CompoundSpare {}

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